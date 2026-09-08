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

# Text exchange files must carry UTF-8 BOM. SQLite is binary and must not be checked as text.
$textFiles=@(
  Get-ChildItem (Join-Path $Root 'data') -Recurse -File | Where-Object { $_.Extension -eq '.json' }
  Get-ChildItem (Join-Path $Root 'exports') -Recurse -File | Where-Object { $_.Extension -eq '.txt' -or $_.Extension -eq '.json' }
)
if($textFiles.Count -eq 0){throw 'No text self-test files found'}
foreach($f in $textFiles){
  $b=[IO.File]::ReadAllBytes($f.FullName)
  if($b.Length -lt 3 -or $b[0] -ne 239 -or $b[1] -ne 187 -or $b[2] -ne 191){throw "Missing BOM: $f"}
}

$db=Join-Path $Root 'data/production-cycle.db'
if(-not (Test-Path $db)){throw 'Missing SQLite database'}
$dbBytes=[IO.File]::ReadAllBytes($db)
$sqliteMagic=[Text.Encoding]::ASCII.GetBytes("SQLite format 3`0")
if($dbBytes.Length -lt $sqliteMagic.Length){throw 'SQLite database is too small'}
for($i=0;$i -lt $sqliteMagic.Length;$i++){
  if($dbBytes[$i] -ne $sqliteMagic[$i]){throw 'Invalid SQLite database header'}
}

if(@(Get-ChildItem (Join-Path $Root 'exports') -Filter *.txt -Recurse).Count -ne 2){throw 'Expected two TXT files'}
if(-not (Test-Path (Join-Path $Root 'temp/webview2'))){throw 'Missing local profile'}
Write-Host 'Native bundled WebView2, external frontend, IPC, JSON, TXT, SQLite and portable data paths: PASS'
