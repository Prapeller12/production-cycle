param([Parameter(Mandatory=$true)][string]$Zip,[switch]$ExpectLegacyFailure)
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true'){throw 'This fault/SMB fixture is CI-only'}
$testRoot=Join-Path $env:RUNNER_TEMP ('Final launcher '+[guid]::NewGuid().ToString('N'))
$payload=Join-Path $testRoot 'Общая папка с пробелами'
New-Item -ItemType Directory -Path $payload -Force|Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $payload
$root=Join-Path $payload 'ProductionCycleNetwork';$profile=Join-Path $testRoot 'Local profile'
New-Item -ItemType Directory -Path $profile|Out-Null
$share='PCFinal'+$PID;$account=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$saved=@{LOCALAPPDATA=$env:LOCALAPPDATA;PRODUCTION_CYCLE_SMOKE_REPORT=$env:PRODUCTION_CYCLE_SMOKE_REPORT;PRODUCTION_CYCLE_STARTUP_REPORT=$env:PRODUCTION_CYCLE_STARTUP_REPORT}
$out=Join-Path (Get-Location) 'dist/final-evidence';New-Item -ItemType Directory -Path $out -Force|Out-Null
try{
 New-SmbShare -Name $share -Path $testRoot -FullAccess $account|Out-Null
 $env:LOCALAPPDATA=$profile
 $launcher="\\$env:COMPUTERNAME\$share\Общая папка с пробелами\ProductionCycleNetwork\app\start.cmd"
 $runs=if($ExpectLegacyFailure){1}else{2}
 for($attempt=1;$attempt -le $runs;$attempt++){
  $name=if($ExpectLegacyFailure){'legacy-cmd'}else{'final-cmd-'+$attempt}
  $env:PRODUCTION_CYCLE_SMOKE_REPORT=Join-Path $testRoot ($name+'.json')
  $env:PRODUCTION_CYCLE_STARTUP_REPORT=Join-Path $testRoot ($name+'-startup.json')
  $stdout=Join-Path $testRoot ($name+'-stdout.txt');$stderr=Join-Path $testRoot ($name+'-stderr.txt')
  $command='/d /c call "'+$launcher+'" --smoke-test <NUL'
  $p=Start-Process -FilePath "$env:SystemRoot/System32/cmd.exe" -ArgumentList $command -WorkingDirectory "$env:SystemRoot/System32" -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
  if(-not $p.WaitForExit(180000)){Stop-Process -Id $p.Id -Force;throw "CMD launcher timed out: $name"}
  $p.WaitForExit();$p.Refresh()
  if($ExpectLegacyFailure){
   if($p.ExitCode -eq 0 -or (Test-Path $env:PRODUCTION_CYCLE_SMOKE_REPORT)){throw 'Old relative launcher did not reproduce its UNC failure'}
   if((Get-Content $stdout -Raw) -notmatch 'Fixed WebView2 Runtime is missing'){throw 'Legacy launcher failed for an unexpected reason'}
   @{ok=$true;expectedFailure=$true;exitCode=$p.ExitCode;launcher=$launcher;workingDirectory="$env:SystemRoot/System32"}|ConvertTo-Json|Set-Content (Join-Path $out ($name+'.json')) -Encoding utf8
   Write-Host 'PASS: reproduced the old CMD working-directory failure over actual SMB'
   continue
  }
  if($p.ExitCode -ne 0 -or -not(Test-Path $env:PRODUCTION_CYCLE_SMOKE_REPORT)){
   throw "CMD launch failed: $name, exit=$($p.ExitCode); $(Get-Content $stderr -Raw); $(Get-Content $stdout -Raw)"
  }
  $r=Get-Content $env:PRODUCTION_CYCLE_SMOKE_REPORT -Raw|ConvertFrom-Json
  if(-not $r.ok -or -not $r.windowVisible -or -not $r.frontendReady -or -not $r.network.enabled){throw "Native client failed: $name $($r.error)"}
  if($r.runtime -match '^\\\\(?!\?\\[A-Za-z]:)' -or $r.profile -match '^\\\\(?!\?\\[A-Za-z]:)'){throw 'CMD fallback left WebView2 on the network'}
  Copy-Item $env:PRODUCTION_CYCLE_SMOKE_REPORT (Join-Path $out ($name+'.json'))
  Copy-Item $env:PRODUCTION_CYCLE_STARTUP_REPORT (Join-Path $out ($name+'-startup.json'))
  Write-Host "PASS: UNC CMD launcher attempt $attempt, visible native UI, $($r.checks.Count) functional checks"
 }
}finally{
 foreach($process in @(Get-CimInstance Win32_Process|Where-Object {$_.Name -in @('production-cycle.exe','msedgewebview2.exe') -and $_.ExecutablePath -like "$profile*"})){Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue}
 Remove-SmbShare -Name $share -Force -ErrorAction SilentlyContinue
 foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}
}
