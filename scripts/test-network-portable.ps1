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
function Wait-VisibleWindow([string]$title,[int]$seconds=90){
  $deadline=[DateTime]::UtcNow.AddSeconds($seconds)
  do {$window=[LaunchWindows]::Find($title);if($window -ne [IntPtr]::Zero){return $window};Start-Sleep -Milliseconds 200}while([DateTime]::UtcNow -lt $deadline)
  throw "No visible window: $title"
}
function Start-NetworkProcess([string]$path,[string]$arguments=''){
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
  $launcher=Start-NetworkProcess $path '--smoke-test'
  $deadline=[DateTime]::UtcNow.AddSeconds(300)
  while(-not $launcher.HasExited -and [DateTime]::UtcNow -lt $deadline){
    $dialog=[LaunchWindows]::Find('Производственный цикл — ошибка запуска')
    if($dialog -ne [IntPtr]::Zero){[LaunchWindows]::Dismiss($dialog) | Out-Null}
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
  if($result.runtime -match '^\\\\(?!\?\\[A-Za-z]:)' -or $result.profile -match '^\\\\(?!\?\\[A-Za-z]:)'){throw 'Runtime/profile were not on a local drive'}
  if(@(Get-ChildItem (Join-Path $root 'shared/sessions') -Filter 'session-*.json').Count -ne 0){throw "Session metadata survived normal exit: $phase"}
  Write-Host "$phase checks: $($result.checks -join '; ')"
}
function Wait-SessionCount([string]$report,[int]$expected){
  $deadline=[DateTime]::UtcNow.AddSeconds(30)
  do {
    try{$result=Get-Content $report -Raw -ErrorAction Stop | ConvertFrom-Json
      if($result.ok -and $result.network.active -and $result.network.occupied -eq $expected){return $result}
      if($result.error){throw "Session probe failed: $($result.error)"}
    }catch{if($_ -match 'Session probe failed'){throw}}
    Start-Sleep -Milliseconds 200
  }while([DateTime]::UtcNow -lt $deadline)
  throw "Session count did not become $expected in $report"
}
function Start-HeldSmoke([string]$path,[string]$phase,[string]$profile=$localProfile){
  $report=Join-Path $testRoot "network-$phase.json"
  $previousProfile=$env:LOCALAPPDATA;$env:LOCALAPPDATA=$profile
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$report;$env:PRODUCTION_CYCLE_SMOKE_HOLD='1'
  try{$launcher=Start-NetworkProcess $path '--smoke-test'}finally{$env:PRODUCTION_CYCLE_SMOKE_HOLD=$null;$env:LOCALAPPDATA=$previousProfile}
  return @{launcher=$launcher;report=$report}
}
$share='PCNetworkTest'+$PID
$account=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$old=@{LOCALAPPDATA=$env:LOCALAPPDATA;USERPROFILE=$env:USERPROFILE;TEMP=$env:TEMP;TMP=$env:TMP}
$mapped=$false
try{
  # This is a real SMB share over TCP/IP, not a local directory simulation.
  New-SmbShare -Name $share -Path $testRoot -FullAccess $account | Out-Null
  $unc="\\127.0.0.1\$share\ProductionCycleNetwork"
  if(-not (Test-Path (Join-Path $unc 'production-cycle.exe'))){throw 'Loopback SMB share is unavailable'}
  $env:LOCALAPPDATA=$localProfile
  Run-FunctionalSmoke $unc 'unc-first-run'
  Run-FunctionalSmoke $unc 'unc-relaunch'
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
  $cache=Join-Path $localProfile "ProductionCycleNetwork/cache/$Version"
  if(-not (Test-Path (Join-Path $root 'shared/database/production-cycle.db'))){throw 'Shared database snapshot missing'}
  if(-not (Test-Path (Join-Path $cache 'runtime/webview2/msedgewebview2.exe'))){throw 'Local Fixed Runtime missing'}
  if(Test-Path (Join-Path $root 'temp/webview2')){throw 'WebView profile created in shared folder'}
}finally{
  foreach($key in $old.Keys){[Environment]::SetEnvironmentVariable($key,$old[$key],'Process')}
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$null;$env:PRODUCTION_CYCLE_SMOKE_HOLD=$null
  Get-CimInstance Win32_Process -Filter "Name='production-cycle.exe'" | Where-Object {$_.ExecutablePath -like "$localProfile*" -or $_.CommandLine -like "*$share*"} | ForEach-Object {Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}
  if($mapped){& net.exe use N: /delete /y | Out-Null}
  Remove-SmbShare -Name $share -Force -ErrorAction SilentlyContinue
}
Write-Host 'PASS: actual SMB/IP first launch, relaunch, mapped drive, redirected profile, visible normal window, visible startup error and native functional UI/IPC.'
