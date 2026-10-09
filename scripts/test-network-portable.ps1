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
function Run-FunctionalSmoke([string]$path,[string]$phase){
  $report=Join-Path $testRoot "network-$phase.json"
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$report
  $launcher=Start-Process -FilePath (Join-Path $path 'production-cycle.exe') -ArgumentList '--smoke-test' -WorkingDirectory $path -PassThru
  if(-not $launcher.WaitForExit(300000)){Stop-Process -Id $launcher.Id -Force -ErrorAction SilentlyContinue;throw "Network $phase timed out"}
  if(-not (Test-Path $report)){throw "Missing report: $phase (exit $($launcher.ExitCode))"}
  $result=Get-Content $report -Raw | ConvertFrom-Json
  if(-not $result.ok -or $launcher.ExitCode -ne 0){throw "Network $phase failed: $($result.error), exit $($launcher.ExitCode)"}
  if(@($result.checks).Count -lt 1 -or -not $result.windowVisible -or -not $result.frontendReady){throw "Network $phase did not verify visible, ready UI"}
  if($result.runtime -match '^\\\\(?!\?\\[A-Za-z]:)' -or $result.profile -match '^\\\\(?!\?\\[A-Za-z]:)'){throw 'Runtime/profile were not on a local drive'}
  Write-Host "$phase checks: $($result.checks -join '; ')"
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
  $launcher=Start-Process -FilePath (Join-Path $unc 'production-cycle.exe') -WorkingDirectory $unc -PassThru
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
    $failed=Start-Process -FilePath (Join-Path $unc 'production-cycle.exe') -ArgumentList '--smoke-test' -WorkingDirectory $unc -PassThru
    $dialog=Wait-VisibleWindow 'Производственный цикл — ошибка запуска' 30
    [LaunchWindows]::PostMessage($dialog,0x0111,[IntPtr]1,[IntPtr]::Zero) | Out-Null
    if(-not $failed.WaitForExit(30000) -or $failed.ExitCode -eq 0){throw 'Failed launch did not exit with an error'}
    $errorReport=Get-Content $env:PRODUCTION_CYCLE_SMOKE_REPORT -Raw | ConvertFrom-Json
    if($errorReport.ok -or $errorReport.error -notmatch 'network.json'){throw 'Expected a configuration error report'}
  }finally{[IO.File]::WriteAllText($networkFile,$networkText,(New-Object Text.UTF8Encoding($false)))}
  $cache=Join-Path $localProfile "ProductionCycleNetwork/cache/$Version"
  if(-not (Test-Path (Join-Path $root 'shared/database/production-cycle.db'))){throw 'Shared database snapshot missing'}
  if(-not (Test-Path (Join-Path $cache 'runtime/webview2/msedgewebview2.exe'))){throw 'Local Fixed Runtime missing'}
  if(Test-Path (Join-Path $root 'temp/webview2')){throw 'WebView profile created in shared folder'}
}finally{
  foreach($key in $old.Keys){[Environment]::SetEnvironmentVariable($key,$old[$key],'Process')}
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=$null
  Get-CimInstance Win32_Process -Filter "Name='production-cycle.exe'" | Where-Object {$_.ExecutablePath -like "$localProfile*" -or $_.CommandLine -like "*$share*"} | ForEach-Object {Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}
  if($mapped){& net.exe use N: /delete /y | Out-Null}
  Remove-SmbShare -Name $share -Force -ErrorAction SilentlyContinue
}
Write-Host 'PASS: actual SMB/IP first launch, relaunch, mapped drive, redirected profile, visible normal window, visible startup error and native functional UI/IPC.'
