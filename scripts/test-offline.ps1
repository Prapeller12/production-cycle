param([Parameter(Mandatory=$true)][string]$Zip)
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true'){throw 'This network-isolation check is only for the disposable GitHub runner.'}
$root=Join-Path $env:RUNNER_TEMP 'Offline check'
Expand-Archive $Zip $root
$app=Join-Path $root 'ProductionCycle'
$programs=@((Join-Path $app 'app/backend/production-cycle.exe'),(Join-Path $app 'runtime/webview2/msedgewebview2.exe'))
$rules=@()
$originalPath=$env:PATH
try {
 if(@(Get-NetFirewallProfile | Where-Object {-not $_.Enabled}).Count -gt 0){throw 'Firewall profiles must be enabled to verify network isolation.'}
 foreach($program in $programs){
  $name='ProductionCycle-CI-'+[guid]::NewGuid().ToString()
  New-NetFirewallRule -Name $name -DisplayName $name -Direction Outbound -Program $program -Action Block -Profile Any | Out-Null
  $rules+=$name
 }
 # Child processes can find only built-in Windows utilities, not installed Python or Node.
 $env:PATH="$env:SystemRoot\System32;$env:SystemRoot"
 & "$PSScriptRoot/test-portable.ps1" -Root $app
 Write-Host 'Offline self-test with outbound firewall blocks and system-only PATH: PASS'
} finally {
 $env:PATH=$originalPath
 foreach($name in $rules){Remove-NetFirewallRule -Name $name -ErrorAction SilentlyContinue}
}
