param(
  [Parameter(Mandatory=$true)][string]$Zip,
  [Parameter(Mandatory=$true)][string]$Version
)
$ErrorActionPreference='Stop'
$testRoot=Join-Path $env:RUNNER_TEMP 'Сетевая проверка программы'
$localProfile=Join-Path $env:RUNNER_TEMP 'Локальный профиль клиента'
if(Test-Path $testRoot){Remove-Item $testRoot -Recurse -Force}
if(Test-Path $localProfile){Remove-Item $localProfile -Recurse -Force}
New-Item -ItemType Directory -Path $testRoot,$localProfile -Force | Out-Null
Expand-Archive $Zip $testRoot
$root=Join-Path $testRoot 'ProductionCycleNetwork'
if(Test-Path (Join-Path $root 'start.cmd')){throw 'CMD must not be exposed in the network package root'}
foreach($required in @('production-cycle.exe','app/start.cmd','frontend/index.html','runtime/webview2/msedgewebview2.exe','config/network.json')){
  if(-not (Test-Path (Join-Path $root $required))){throw "Missing network package item: $required"}
}
$config=Get-Content (Join-Path $root 'config/network.json') -Raw | ConvertFrom-Json
if(-not $config.enabled -or $config.maxClients -ne 3 -or $config.version -ne $Version){throw 'Invalid network configuration'}
$oldLocalAppData=$env:LOCALAPPDATA
try{
  $env:LOCALAPPDATA=$localProfile
  $launcher=Start-Process -FilePath (Join-Path $root 'production-cycle.exe') -ArgumentList '--smoke-test' -WorkingDirectory $root -PassThru
  if(-not $launcher.WaitForExit(30000)){Stop-Process -Id $launcher.Id -Force -ErrorAction SilentlyContinue;throw 'Network launcher timed out'}
  if($launcher.ExitCode -ne 0){throw 'Network launcher failed'}
  $cache=Join-Path $localProfile "ProductionCycleNetwork/cache/$Version"
  $report=Join-Path $cache 'temp/self-test.json'
  $deadline=(Get-Date).AddSeconds(150)
  while(-not (Test-Path $report) -and (Get-Date) -lt $deadline){Start-Sleep -Milliseconds 500}
  if(-not (Test-Path $report)){throw 'Local network client smoke report was not created'}
  $result=Get-Content $report -Raw | ConvertFrom-Json
  if(-not $result.ok){throw "Network client self-test failed: $($result.error)"}
  if(-not (Test-Path (Join-Path $root 'shared/database/production-cycle.db'))){throw 'Shared database snapshot was not created'}
  if(-not (Test-Path (Join-Path $cache 'runtime/webview2/msedgewebview2.exe'))){throw 'Fixed WebView2 was not copied to the local client cache'}
  if(Test-Path (Join-Path $root 'temp/webview2')){throw 'WebView profile must not be created in the shared folder'}
} finally {$env:LOCALAPPDATA=$oldLocalAppData}
Write-Host 'Network launcher, local runtime/profile, shared database snapshot and native UI/IPC: PASS'
