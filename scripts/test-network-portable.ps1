param(
  [Parameter(Mandatory=$true)][string]$Zip,
  [Parameter(Mandatory=$true)][string]$Version
)
$ErrorActionPreference='Stop'
$testRoot=Join-Path $env:RUNNER_TEMP 'Сетевая проверка программы'
$localProfile=Join-Path $env:RUNNER_TEMP 'Локальный профиль клиента'
foreach($path in @($testRoot,$localProfile)){if(Test-Path $path){Remove-Item $path -Recurse -Force};New-Item -ItemType Directory -Path $path -Force | Out-Null}
Expand-Archive $Zip $testRoot
$root=Join-Path $testRoot 'ProductionCycleNetwork'
(Get-Item (Join-Path $root 'frontend/index.html')).IsReadOnly=$true
if(Test-Path (Join-Path $root 'start.cmd')){throw 'CMD must not be exposed in the network package root'}
foreach($required in @('production-cycle.exe','app/start.cmd','frontend/index.html','runtime/webview2/msedgewebview2.exe','config/network.json')){
  if(-not (Test-Path (Join-Path $root $required))){throw "Missing network package item: $required"}
}
$config=Get-Content (Join-Path $root 'config/network.json') -Raw | ConvertFrom-Json
if(-not $config.enabled -or $config.maxClients -ne 3 -or $config.version -ne $Version){throw 'Invalid network configuration'}
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class LaunchWindows {
  public delegate bool Callback(IntPtr window, IntPtr param);
  [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback,IntPtr param);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window,StringBuilder text,int max);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window,uint message,IntPtr w,IntPtr l);
  [DllImport("user32.dll")] static extern IntPtr GetDlgItem(IntPtr dialog,int id);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr window);
  [DllImport("user32.dll",SetLastError=true)] static extern IntPtr SendMessageTimeoutW(IntPtr window,uint msg,IntPtr w,IntPtr l,uint flags,uint timeout,out UIntPtr result);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window,out uint process);
  public static uint Owner(IntPtr window){uint process;GetWindowThreadProcessId(window,out process);return process;}
  public static bool Dismiss(IntPtr dialog) {
    var button=GetDlgItem(dialog,1);
    SetForegroundWindow(dialog);
    if(button!=IntPtr.Zero)PostMessage(button,0x00F5,IntPtr.Zero,IntPtr.Zero);
    UIntPtr result;
    return SendMessageTimeoutW(dialog,0x0010,IntPtr.Zero,IntPtr.Zero,2,500,out result)!=IntPtr.Zero;
  }
  public static IntPtr Find(string title) {
    IntPtr result=IntPtr.Zero;
    EnumWindows((window,param)=>{var text=new StringBuilder(512);GetWindowText(window,text,512);if(IsWindowVisible(window)&&text.ToString()==title){result=window;return false;}return true;},IntPtr.Zero);
    return result;
  }
}
'@
Add-Type -Path (Join-Path $PSScriptRoot 'launch-limited-token.cs')
. (Join-Path $PSScriptRoot 'trace-standard-user-startup.ps1')
$script:LimitedLaunch=$false
function Wait-VisibleWindow([string]$title,[int]$seconds=90){
  $deadline=[DateTime]::UtcNow.AddSeconds($seconds)
  do {$window=[LaunchWindows]::Find($title);if($window -ne [IntPtr]::Zero){return $window};Start-Sleep -Milliseconds 200}while([DateTime]::UtcNow -lt $deadline)
  throw "No visible window: $title"
}
function Start-NetworkProcess([string]$path,[string]$arguments=''){
  if($script:LimitedLaunch){return [LimitedLaunch]::Start((Join-Path $path 'production-cycle.exe'),$arguments,$path)}
  # CreateProcess, like the production local launcher. ShellExecute can stop
  # unattended tests at an Explorer trust prompt for an IP-based UNC path.
  $info=[Diagnostics.ProcessStartInfo]::new()
  $info.FileName=Join-Path $path 'production-cycle.exe';$info.WorkingDirectory=$path
  $info.UseShellExecute=$false;$info.Arguments=$arguments
  return [Diagnostics.Process]::Start($info)
}
function Run-FunctionalSmoke([string]$path,[string]$phase){
  $report=Join-Path $testRoot "network-$phase.json"
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$report
  $env:PRODUCTION_CYCLE_STARTUP_REPORT=Join-Path $testRoot "network-launch-$phase.json"
  $launcher=Start-NetworkProcess $path '--smoke-test'
  $deadline=[DateTime]::UtcNow.AddSeconds(300)
  $sampleAt=[DateTime]::UtcNow.AddSeconds(10);$sampled=$false
  while(-not $launcher.HasExited -and [DateTime]::UtcNow -lt $deadline){
    $dialog=[LaunchWindows]::Find('Производственный цикл — ошибка запуска')
    if($dialog -ne [IntPtr]::Zero){[LaunchWindows]::Dismiss($dialog) | Out-Null}
    if($script:LimitedLaunch -and -not $sampled -and [DateTime]::UtcNow -ge $sampleAt){
      $sampled=$true
      $processes=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -in @('production-cycle.exe','msedgewebview2.exe')} | Select-Object Name,ProcessId,ParentProcessId,SessionId,ExecutablePath,CommandLine,@{Name='Owner';Expression={try{$owner=Invoke-CimMethod -InputObject $_ -MethodName GetOwner;"$($owner.Domain)\$($owner.User)"}catch{"$($_.Exception.Message)"}}})
      $profiles=@(foreach($item in $processes){
        if($item.CommandLine -match '--user-data-dir=(?:"([^"]+)"|(\S+))'){
          $folder=if($Matches[1]){$Matches[1]}else{$Matches[2]}
          try{@{path=$folder;sddl=(Get-Acl -LiteralPath $folder).Sddl;entries=@(Get-ChildItem -LiteralPath $folder -Force | Select-Object Name,Attributes,Length)}}catch{@{path=$folder;error="$($_.Exception.Message)"}}
        }
      })
      @{processes=$processes;profiles=$profiles} | ConvertTo-Json -Depth 7 | Set-Content (Join-Path $testRoot 'network-standard-user-processes.json') -Encoding utf8
    }
    Start-Sleep -Milliseconds 200
    $launcher.Refresh()
  }
  if(-not $launcher.HasExited){
    Stop-Process -Id $launcher.Id -Force -ErrorAction SilentlyContinue
    $detail=if(Test-Path $report){Get-Content $report -Raw}else{'No startup or functional report'}
    throw "Network $phase timed out: $detail"
  }
  $launcher.WaitForExit()
  if(-not (Test-Path $report)){throw "Missing report: $phase (exit $($launcher.ExitCode))"}
  $result=Get-Content $report -Raw | ConvertFrom-Json
  if(-not $result.ok -or $launcher.ExitCode -ne 0){throw "Network $phase failed: $($result.error), exit $($launcher.ExitCode)"}
  if(@($result.checks).Count -lt 1 -or -not $result.windowVisible -or -not $result.frontendReady){throw "Network $phase did not verify visible, ready UI"}
  $launch=Get-Content $env:PRODUCTION_CYCLE_STARTUP_REPORT -Raw | ConvertFrom-Json
  if($launch.state -ne 'ready' -or -not $launch.client.windowVisible -or -not $launch.client.frontendReady){throw "Launcher returned before a ready user window: $phase"}
  if($result.runtime -match '^\\\\(?!\?\\[A-Za-z]:)' -or $result.profile -match '^\\\\(?!\?\\[A-Za-z]:)'){throw 'Runtime/profile were not on a local drive'}
  if(@(Get-ChildItem (Join-Path $root 'shared/sessions') -Filter 'session-*.json').Count -ne 0){throw "Session metadata survived normal exit: $phase"}
  Write-Host "$phase checks: $($result.checks -join '; ')"
}
function Wait-SessionCount([string]$report,[int]$expected){
  # A new WebView must finish its functional smoke before it starts writing the
  # held-client report. Keep its startup allowance separate from the 30-second
  # bound for presence changes in an already running client.
  $started=[DateTime]::UtcNow
  $startupDeadline=$started.AddSeconds(90)
  $counterDeadline=$null
  $lastDetail='Report has not been created'
  do {
    try{$lastDetail=Get-Content $report -Raw -ErrorAction Stop;$result=$lastDetail | ConvertFrom-Json
      if($result.error){throw "Session probe failed: $($result.error)"}
      $ready=$result.ok -and $result.frontendReady -and $result.windowVisible -and $result.network.active -and @($result.checks).Count -ge 1
      if($ready -and $null -eq $counterDeadline){
        $counterDeadline=[DateTime]::UtcNow.AddSeconds(30)
        Write-Host "Held client ready after $([Math]::Round(([DateTime]::UtcNow-$started).TotalSeconds,1)) seconds: $report"
      }
      if($ready -and $result.network.occupied -eq $expected){return $result}
    }catch{if($_ -match 'Session probe failed'){throw}}
    Start-Sleep -Milliseconds 200
    $deadline=if($null -eq $counterDeadline){$startupDeadline}else{$counterDeadline}
  }while([DateTime]::UtcNow -lt $deadline)
  if($null -eq $counterDeadline){throw "Client did not finish its smoke within 90 seconds: $report. Last report: $lastDetail"}
  throw "Session count did not become $expected within 30 seconds after readiness: $report. Last report: $lastDetail"
}
function Start-HeldSmoke([string]$path,[string]$phase,[string]$profile=$localProfile){
  $report=Join-Path $testRoot "network-$phase.json"
  $previousProfile=$env:LOCALAPPDATA;$env:LOCALAPPDATA=$profile
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$report;$env:PRODUCTION_CYCLE_SMOKE_HOLD='1'
  try{$launcher=Start-NetworkProcess $path '--smoke-test'}finally{$env:PRODUCTION_CYCLE_SMOKE_HOLD=$null;$env:LOCALAPPDATA=$previousProfile}
  return @{launcher=$launcher;report=$report}
}
function Run-ExpectedLaunchFailure([string]$path,[string]$phase,[string]$pattern){
  $report=Join-Path $testRoot "network-$phase.json"
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$report
  $env:PRODUCTION_CYCLE_STARTUP_REPORT=Join-Path $testRoot "network-launch-$phase.json"
  $process=Start-NetworkProcess $path '--smoke-test'
  $deadline=[DateTime]::UtcNow.AddSeconds(180)
  $visible=$false
  while(-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline){
    $dialog=[LaunchWindows]::Find('Производственный цикл — ошибка запуска')
    if($dialog -ne [IntPtr]::Zero){$visible=$true;[LaunchWindows]::Dismiss($dialog) | Out-Null}
    Start-Sleep -Milliseconds 100;$process.Refresh()
  }
  if(-not $process.HasExited){Stop-Process -Id $process.Id -Force;throw "Expected failure timed out: $phase"}
  $process.WaitForExit()
  $value=Get-Content $report -Raw | ConvertFrom-Json
  if(-not $visible -or $process.ExitCode -eq 0 -or $value.ok -or $value.error -notmatch $pattern){throw "Wrong failure: $phase $($value.error)"}
  Write-Host "PASS expected startup failure: $phase"
}
function With-DeniedPath([string]$path,[string]$permissions,[scriptblock]$body){
  $acl=Get-Acl $path;$saved=$acl.Sddl
  try{
    $inherit=if((Get-Item $path).PSIsContainer){[Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'}else{[Security.AccessControl.InheritanceFlags]::None}
    $rule=[Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.AccessControl.FileSystemRights]$permissions,$inherit,[Security.AccessControl.PropagationFlags]::None,[Security.AccessControl.AccessControlType]::Deny)
    $acl.AddAccessRule($rule);Set-Acl -Path $path -AclObject $acl
    & $body
  }finally{$restore=Get-Acl $path;$restore.SetSecurityDescriptorSddlForm($saved);Set-Acl -Path $path -AclObject $restore}
}
$share='PCNetworkTest'+$PID
$account=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$limitedName='pc_launch_'+$PID
$limitedAccount=$env:COMPUTERNAME+'\'+$limitedName
$limitedPassword='Pc!'+[Guid]::NewGuid().ToString('N')+'a9'
$limitedCreated=$false
$old=@{LOCALAPPDATA=$env:LOCALAPPDATA;USERPROFILE=$env:USERPROFILE;TEMP=$env:TEMP;TMP=$env:TMP}
$mapped=$false
try{
  # This is a real SMB share over TCP/IP, not a local directory simulation.
  New-SmbShare -Name $share -Path $testRoot -FullAccess $account | Out-Null
  $unc="\\127.0.0.1\$share\ProductionCycleNetwork"
  if(-not (Test-Path (Join-Path $unc 'production-cycle.exe'))){throw 'Loopback SMB share is unavailable'}
  $env:LOCALAPPDATA=$localProfile
  # A real standard account with its profile loaded, not a synthetic restricted
  # token (which WebView2 does not support). Only the isolated CI fixtures and
  # this runner's desktop receive access; production permissions are untouched.
  New-LocalUser -Name $limitedName -Password (ConvertTo-SecureString $limitedPassword -AsPlainText -Force) -AccountNeverExpires -PasswordNeverExpires | Out-Null
  $limitedCreated=$true
  Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $limitedName
  Grant-SmbShareAccess -Name $share -AccountName $limitedAccount -AccessRight Full -Force | Out-Null
  foreach($fixture in @($testRoot,$localProfile)){
    $acl=Get-Acl $fixture
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($limitedAccount,'Modify','ContainerInherit, ObjectInherit','None','Allow'))
    Set-Acl $fixture $acl
  }
  Start-Service seclogon
  $standardError=$null;$trace=$null
  try{$trace=Start-StandardUserTrace}catch{Write-Warning "Optional CI trace unavailable: $_"}
  $script:LimitedLaunch=$true
  try{[LimitedLaunch]::Configure($limitedName,$limitedPassword);Run-FunctionalSmoke $unc 'limited-user'}catch{
    $standardError=$_;Write-Warning "Standard-user startup failed; continuing other mandatory scenarios: $_"
  }finally{
    [LimitedLaunch]::Restore();$script:LimitedLaunch=$false
    try{Stop-StandardUserTrace $trace (Join-Path $testRoot 'network-standard-user-file-events.csv')}catch{Write-Warning "Optional CI trace export failed: $_"}
  }
  Run-FunctionalSmoke $unc 'unc-first-run'
  Run-FunctionalSmoke $unc 'unc-relaunch'
  # A stale or damaged cache must be rebuilt without overwriting a locked EXE.
  $initial=Get-Content (Join-Path $testRoot 'network-launch-unc-first-run.json') -Raw | ConvertFrom-Json
  $sentinel=Join-Path $initial.cache 'data/keep-on-rebuild.txt'
  New-Item -ItemType Directory -Path (Split-Path $sentinel) -Force | Out-Null
  [IO.File]::WriteAllText($sentinel,'preserve this old cache')
  [IO.File]::AppendAllText((Join-Path $initial.cache 'frontend/index.html'),'damaged cache')
  $locked=[IO.File]::Open((Join-Path $initial.cache 'production-cycle.exe'),'Open','Read','ReadWrite')
  try{Run-FunctionalSmoke $unc 'damaged-locked-cache'}finally{$locked.Dispose()}
  $rebuilt=Get-Content (Join-Path $testRoot 'network-launch-damaged-locked-cache.json') -Raw | ConvertFrom-Json
  if($rebuilt.cache -eq $initial.cache -or (Get-Content $sentinel -Raw) -ne 'preserve this old cache'){throw 'Cache repair overwrote or deleted old working files'}
  & net.exe use N: "\\127.0.0.1\$share" /persistent:no | Out-Null
  if($LASTEXITCODE -ne 0){throw 'Cannot map SMB test drive'};$mapped=$true
  Run-FunctionalSmoke 'N:\ProductionCycleNetwork' 'mapped-relaunch'
  # Domain profiles may redirect LOCALAPPDATA/USERPROFILE to a network share.
  # The runtime must fall back to a real local TEMP and still open the full UI.
  $env:LOCALAPPDATA="\\127.0.0.1\$share\redirected-profile"
  $env:USERPROFILE="\\127.0.0.1\$share\redirected-user"
  $env:TEMP=Join-Path $localProfile 'fallback-temp';$env:TMP=$env:TEMP
  New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
  Run-FunctionalSmoke $unc 'redirected-profile'
  $env:LOCALAPPDATA=$localProfile;$env:USERPROFILE=$old.USERPROFILE
  # Actual NTFS access-denied failures. Restore ACLs after each private fixture.
  $databaseHash=(Get-FileHash (Join-Path $root 'shared/database/production-cycle.db')).Hash
  With-DeniedPath (Join-Path $root 'shared/drafts') 'CreateFiles' {
    Run-ExpectedLaunchFailure $unc 'denied-shared-write' '(?s)shared[\\/]drafts.*Код ОС: 5'
  }
  if((Get-FileHash (Join-Path $root 'shared/database/production-cycle.db')).Hash -ne $databaseHash){throw 'Failed startup changed the shared database'}
  $freshProfile=Join-Path $testRoot 'Профиль проверки доступа';New-Item -ItemType Directory -Path $freshProfile -Force | Out-Null
  $env:LOCALAPPDATA=$freshProfile;$env:USERPROFILE=$freshProfile
  $env:TEMP=Join-Path $freshProfile 'SourceTemp';$env:TMP=$env:TEMP
  New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
  With-DeniedPath (Join-Path $root 'frontend/index.html') 'ReadData' {
    Run-ExpectedLaunchFailure $unc 'denied-source-read' '(?s)frontend[\\/]index.html.*Код ОС: 5'
  }
  Run-FunctionalSmoke $unc 'before-denied-execute'
  $execute=Get-Content (Join-Path $testRoot 'network-launch-before-denied-execute.json') -Raw | ConvertFrom-Json
  With-DeniedPath (Join-Path $execute.cache 'production-cycle.exe') 'ExecuteFile' {
    Run-FunctionalSmoke $unc 'denied-execute-fallback'
  }
  $executeFallback=Get-Content (Join-Path $testRoot 'network-launch-denied-execute-fallback.json') -Raw | ConvertFrom-Json
  if(-not $executeFallback.fallbackUsed -or ($executeFallback.previousAttempts -join ' ') -notmatch 'Код ОС: 5'){throw 'Denied EXE execution was not recovered through another local path'}
  # A blocked primary profile is recoverable through a permitted local TEMP.
  $fallback=Join-Path $testRoot 'Разрешённый локальный TEMP';New-Item -ItemType Directory -Path $fallback -Force | Out-Null
  $env:TEMP=$fallback;$env:TMP=$fallback
  With-DeniedPath $freshProfile 'CreateDirectories, CreateFiles' {
    Run-FunctionalSmoke $unc 'denied-profile-fallback'
  }
  $fallbackLaunch=Get-Content (Join-Path $testRoot 'network-launch-denied-profile-fallback.json') -Raw | ConvertFrom-Json
  if($fallbackLaunch.cache -notlike "*$([IO.Path]::GetFileName($fallback))*"){throw 'Blocked profile did not use local fallback'}
  $env:LOCALAPPDATA=$localProfile;$env:USERPROFILE=$old.USERPROFILE;$env:TEMP=$old.TEMP;$env:TMP=$old.TMP
  $allBlocked=Join-Path $testRoot 'Все локальные папки запрещены';New-Item -ItemType Directory -Path $allBlocked -Force | Out-Null
  $env:LOCALAPPDATA=$allBlocked;$env:USERPROFILE=$allBlocked;$env:TEMP=$allBlocked;$env:TMP=$allBlocked
  With-DeniedPath $allBlocked 'CreateDirectories, CreateFiles' {
    Run-ExpectedLaunchFailure $unc 'all-local-paths-denied' '(?s)Нет доступной локальной папки клиента.*Код ОС: 5'
  }
  $env:LOCALAPPDATA=$localProfile;$env:USERPROFILE=$old.USERPROFILE;$env:TEMP=$old.TEMP;$env:TMP=$old.TMP
  $networkConfig=Join-Path $root 'config/network.json';$hiddenConfig=$networkConfig+'.saved'
  Move-Item $networkConfig $hiddenConfig
  try{Run-ExpectedLaunchFailure $unc 'missing-network-config' 'network.json'}finally{Move-Item $hiddenConfig $networkConfig}
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$null
  # A normal launch (without self-test) must display an actual top-level window.
  $launcher=Start-NetworkProcess $unc
  $window=Wait-VisibleWindow 'Производственный цикл — self-test v1'
  $clients=@(Get-CimInstance Win32_Process -Filter "Name='production-cycle.exe'" | Where-Object {$_.CommandLine -match '--network-client'})
  if($clients.Count -ne 1 -or $clients[0].ExecutablePath -notlike "$localProfile*"){throw 'Normal UNC launch did not create one local client'}
  $clientId=$clients[0].ProcessId
  $clientProcess=Get-Process -Id $clientId
  [LaunchWindows]::PostMessage($window,0x0010,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
  if(-not $clientProcess.WaitForExit(30000)){throw 'Normal client did not close'}
  @{ok=$true;phase='normal-visible-window';transport='SMB';path=$unc;clientPath=$clients[0].ExecutablePath} | ConvertTo-Json | Set-Content (Join-Path $testRoot 'network-window.json')
  # Verify errors before WebView2 are visible and leave no hidden client alive.
  $networkFile=Join-Path $root 'config/network.json';$networkText=Get-Content $networkFile -Raw
  try{
    Set-Content $networkFile '{invalid json' -Encoding utf8
    $env:PRODUCTION_CYCLE_SMOKE_REPORT=Join-Path $testRoot 'network-visible-error.json'
    $failed=Start-NetworkProcess $unc '--smoke-test'
    $dialog=Wait-VisibleWindow 'Производственный цикл — ошибка запуска' 30
    if([LaunchWindows]::Owner($dialog) -ne $failed.Id){throw 'Startup error belongs to a different process'}
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    do {
      [LaunchWindows]::Dismiss($dialog) | Out-Null
      Start-Sleep -Milliseconds 200
      $failed.Refresh()
    }while(-not $failed.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if(-not $failed.HasExited){throw 'Visible startup error dialog did not close'}
    $failed.WaitForExit()
    if($failed.ExitCode -eq 0){throw "Failed launch returned success; report: $(Get-Content $env:PRODUCTION_CYCLE_SMOKE_REPORT -Raw)"}
    $errorReport=Get-Content $env:PRODUCTION_CYCLE_SMOKE_REPORT -Raw | ConvertFrom-Json
    if($errorReport.ok -or $errorReport.error -notmatch 'network.json'){throw 'Expected a configuration error report'}
  }finally{[IO.File]::WriteAllText($networkFile,$networkText,(New-Object Text.UTF8Encoding($false)))}
  # Two real EXE clients on SMB. The remaining client's reported count must
  # decrease after both a normal window close and a forced process kill.
  $first=Start-HeldSmoke $unc 'session-first'
  $firstResult=Wait-SessionCount $first.report 1
  $second=Start-HeldSmoke $unc 'session-second'
  $secondResult=Wait-SessionCount $second.report 2
  if($firstResult.profile -eq $secondResult.profile -or $firstResult.database -eq $secondResult.database){throw 'Two windows of one Windows profile share a writable data/profile directory'}
  Wait-SessionCount $first.report 2 | Out-Null
  # Find by PID, since both client windows have the same title.
  $secondProcess=Get-Process -Id $secondResult.processId
  if(-not $secondProcess.CloseMainWindow()){throw 'Cannot close second client window'}
  if(-not $secondProcess.WaitForExit(30000)){throw 'Second client did not exit normally'}
  Wait-SessionCount $first.report 1 | Out-Null
  $second=Start-HeldSmoke $unc 'session-forced'
  $secondResult=Wait-SessionCount $second.report 2
  if($firstResult.profile -eq $secondResult.profile -or $firstResult.database -eq $secondResult.database){throw 'Two windows of one Windows profile share a writable data/profile directory'}
  Wait-SessionCount $first.report 2 | Out-Null
  Stop-Process -Id $second.launcher.Id -Force -ErrorAction SilentlyContinue
  Stop-Process -Id $secondResult.processId -Force
  Wait-SessionCount $first.report 1 | Out-Null
  # Release of the remaining client must leave no abandoned session JSON.
  $firstProcess=Get-Process -Id $firstResult.processId
  if(-not $firstProcess.CloseMainWindow()){throw 'Cannot close first client window'}
  if(-not $firstProcess.WaitForExit(30000)){throw 'First client did not exit normally'}
  if(@(Get-ChildItem (Join-Path $root 'shared/sessions') -Filter 'session-*.json').Count -ne 0){throw 'Session files were not cleaned after close'}
  Run-FunctionalSmoke $unc 'after-forced-relaunch'
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$null
  $lastLaunch=Get-Content (Join-Path $testRoot 'network-launch-after-forced-relaunch.json') -Raw | ConvertFrom-Json
  $cache=$lastLaunch.cache
  if(-not (Test-Path (Join-Path $root 'shared/database/production-cycle.db'))){throw 'Shared database snapshot missing'}
  if(-not (Test-Path (Join-Path $cache 'runtime/webview2/msedgewebview2.exe'))){throw 'Local Fixed Runtime missing'}
  if(Test-Path (Join-Path $root 'temp/webview2')){throw 'WebView profile created in shared folder'}
  if($null -ne $standardError){throw $standardError}
}finally{
  foreach($key in $old.Keys){[Environment]::SetEnvironmentVariable($key,$old[$key],'Process')}
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$null;$env:PRODUCTION_CYCLE_SMOKE_HOLD=$null;$env:PRODUCTION_CYCLE_STARTUP_REPORT=$null
  Get-CimInstance Win32_Process -Filter "Name='production-cycle.exe'" | Where-Object {$_.ExecutablePath -like "$localProfile*" -or $_.CommandLine -like "*$share*"} | ForEach-Object {Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}
  if($mapped){& net.exe use N: /delete /y | Out-Null}
  Remove-SmbShare -Name $share -Force -ErrorAction SilentlyContinue
  if($limitedCreated){Remove-LocalUser -Name $limitedName -ErrorAction SilentlyContinue}
}
Write-Host 'PASS: actual SMB/IP first launch, relaunch, mapped drive, redirected profile, visible normal window, visible startup error and native functional UI/IPC.'
