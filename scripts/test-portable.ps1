param([Parameter(Mandatory=$true)][string]$Root)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path $Root).Path
$exe=Join-Path $Root 'app/backend/production-cycle.exe'
$report=Join-Path $Root 'temp/self-test.json'
$p=Start-Process -FilePath $exe -ArgumentList '--smoke-test' -WorkingDirectory $env:TEMP -PassThru
if (-not $p.WaitForExit(120000)) {
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  throw 'Native UI/IPC smoke-test timed out'
}
if ($p.ExitCode -ne 0) {if(Test-Path $report){Get-Content $report};throw 'Native self-test failed'}
$r=Get-Content $report -Raw | ConvertFrom-Json
if (-not $r.ok) {throw "Native UI/IPC self-test failed: $($r.error)"}
foreach($folder in @('data','exports')) {
 $files=@(Get-ChildItem (Join-Path $Root $folder) -Recurse -File)
 if ($files.Count -eq 0) {throw "No test files in $folder"}
 foreach($f in $files) {
  $b=[IO.File]::ReadAllBytes($f.FullName)
  if($b.Length -lt 3 -or $b[0] -ne 239 -or $b[1] -ne 187 -or $b[2] -ne 191){throw "Missing BOM: $f"}
 }
}
if(@(Get-ChildItem (Join-Path $Root 'exports') -Filter *.txt -Recurse).Count -ne 2){throw 'Expected two TXT files'}
if(-not (Test-Path (Join-Path $Root 'temp/webview2'))){throw 'Missing local profile'}
Write-Host 'Native bundled WebView2, external frontend, IPC, JSON, TXT and portable data paths: PASS'
