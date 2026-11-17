param(
  [Parameter(Mandatory=$true)][string]$Zip,
  [Parameter(Mandatory=$true)][string]$Version,
  [string]$ReportDirectory='./dist/statistics-shared-qa'
)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true'){
  throw 'This isolated Windows/SMB acceptance fixture must run on Windows CI'
}
$repo=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=[IO.Path]::GetFullPath($ReportDirectory)
New-Item -ItemType Directory -Path $out -Force | Out-Null
$testRoot=Join-Path $env:RUNNER_TEMP ('Statistics shared preferences '+[guid]::NewGuid().ToString('N'))
$payload=Join-Path $testRoot 'Shared package'
$clients=Join-Path $testRoot 'Independent clients'
$share='PCStatistics'+$PID
$account=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$shareCreated=$false
$resultFile=Join-Path $out 'statistics-shared-preferences.json'
Remove-Item -LiteralPath $resultFile -Force -ErrorAction SilentlyContinue
try{
  New-Item -ItemType Directory -Path $payload,$clients -Force | Out-Null
  $zipPath=(Resolve-Path -LiteralPath $Zip).Path
  Expand-Archive -LiteralPath $zipPath -DestinationPath $payload
  $root=Join-Path $payload 'ProductionCycleNetwork'
  foreach($relative in @('production-cycle.exe','launch-manifest.json','frontend/index.html','runtime/webview2/msedgewebview2.exe','config/network.json')){
    if(-not (Test-Path -LiteralPath (Join-Path $root $relative) -PathType Leaf)){throw "Missing fresh ZIP payload: $relative"}
  }
  $config=Get-Content -LiteralPath (Join-Path $root 'config/network.json') -Raw | ConvertFrom-Json
  $manifest=Get-Content -LiteralPath (Join-Path $root 'launch-manifest.json') -Raw | ConvertFrom-Json
  if(-not $config.enabled -or $config.version -ne $Version -or $manifest.version -ne $Version){throw 'Fresh ZIP network version does not match this CI build'}
  if(Test-Path -LiteralPath (Join-Path $root 'shared/preferences/workspace.json')){throw 'ZIP unexpectedly contains an existing personal-preferences workspace'}
  # Real SMB/UNC over loopback. These are independent client caches on one CI
  # host, not a claim that this test used two physical computers.
  New-SmbShare -Name $share -Path $payload -FullAccess $account | Out-Null
  $shareCreated=$true
  $unc="\\127.0.0.1\$share\ProductionCycleNetwork"
  $null=[IO.File]::ReadAllBytes((Join-Path $unc 'config/network.json'))
  $connections=@(Get-SmbConnection | Where-Object {$_.ShareName -eq $share} | Select-Object ServerName,ShareName,Dialect,NumOpens)
  if($connections.Count -lt 1){throw 'Windows did not report a real SMB connection for the UNC fixture'}
  $profiles=@((Join-Path $clients 'Client A'),(Join-Path $clients 'Client B'))
  foreach($profile in $profiles){New-Item -ItemType Directory -Path $profile -Force | Out-Null}
  $fixture=[ordered]@{
    uncRoot=$unc
    clientProfiles=$profiles
    outputDirectory=$out
    sourceVersion=$Version
    zipSha256=(Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    hostOs=(Get-CimInstance Win32_OperatingSystem).Caption
    smbConnections=$connections
  }
  $fixturePath=Join-Path $testRoot 'statistics-shared-fixture.json'
  [IO.File]::WriteAllText($fixturePath,($fixture | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
  & node (Join-Path $repo 'tests/statistics-shared-preferences-native.cjs') $fixturePath
  if($LASTEXITCODE -ne 0){throw "Native shared Statistics acceptance failed; see $resultFile"}
  $result=Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
  if(-not $result.ok){throw 'Native shared Statistics runner did not confirm success'}
  Write-Host 'PASS: fresh ZIP, real SMB/UNC, independent local clients, restart/cache repair, profile isolation, CAS and SMB failure recovery'
}catch{
  if(-not (Test-Path -LiteralPath $resultFile)){
    [IO.File]::WriteAllText($resultFile,(@{ok=$false;phase='Windows SMB fixture';error=$_.Exception.Message} | ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
  }
  throw
}finally{
  # Only this test's client trees are eligible for forced process cleanup.
  foreach($process in @(Get-CimInstance Win32_Process | Where-Object {$_.Name -in @('production-cycle.exe','msedgewebview2.exe') -and $_.ExecutablePath -like "$clients\*"})){
    Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
  }
  if($shareCreated){Remove-SmbShare -Name $share -Force -ErrorAction SilentlyContinue}
}
