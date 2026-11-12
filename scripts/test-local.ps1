param([Parameter(Mandatory=$true)][string]$Root)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path -LiteralPath $Root).Path
$report=Join-Path $Root 'temp/self-test.json'
for($attempt=1;$attempt -le 2;$attempt++){
 Remove-Item $report -ErrorAction SilentlyContinue
 $process=Start-Process -FilePath "$Root/production-cycle.exe" -ArgumentList '--smoke-test' -WorkingDirectory $env:TEMP -PassThru
 if(-not $process.WaitForExit(180000)){Stop-Process -Id $process.Id -Force;throw 'Local UI/IPC test timed out'}
 if(-not(Test-Path $report)){throw 'Native test report missing'}
 $r=Get-Content $report -Raw | ConvertFrom-Json
 if(-not $r.ok -or $process.ExitCode -ne 0){throw "Local UI/IPC test failed: $($r.error)"}
 if($r.network.enabled){throw 'Network runtime unexpectedly enabled'}
 Copy-Item $report "$Root/temp/self-test-$attempt.json"
 Write-Host "Native local EXE attempt $attempt passed ($($r.checks.Count) checks)"
}
$template=Get-ChildItem "$Root/exports" -Filter '*.xlsx' | Select-Object -First 1
if(-not $template -or (Get-FileHash $template.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne '1c882f36cdc917d17eccbaf462588c4697a1eb4a4d1bf438f01d967979a2d2c2'){throw 'Native template binary differs from approved workbook'}
if(-not(Test-Path "$Root/data/drafts/user-1.json")){throw 'Local draft missing'}
if(Test-Path "$Root/config/network.json"){throw 'SMB configuration must not be shipped'}
if(Test-Path "$Root/server"){throw 'Server must not be shipped'}
