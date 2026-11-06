param([Parameter(Mandatory=$true)][string]$Version,[string]$OutputPath='./dist/server')
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$destination=Join-Path $OutputPath 'ProductionCycleServer'
if(Test-Path $destination){Remove-Item -LiteralPath $destination -Recurse -Force}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item (Join-Path $repo 'server/target/release/production-cycle-server.exe') $destination
foreach($folder in @('frontend','config','scripts/server')){
 $target=Join-Path $destination $folder;New-Item -ItemType Directory -Path $target -Force | Out-Null
 if($folder -eq 'config'){Copy-Item (Join-Path $repo 'config/server.json') $target}else{Copy-Item (Join-Path $repo "$folder/*") $target -Recurse}
}
Copy-Item (Join-Path $repo 'docs/server-browser-guide.md') (Join-Path $destination 'ПРОЧИТАЙТЕ.md')
Copy-Item (Join-Path $repo 'docs/server-browser-acceptance.md') (Join-Path $destination 'ПРОВЕРКИ.md')
Get-ChildItem (Join-Path $destination 'scripts/server') -Filter '*.ps1' | ForEach-Object {
 $text=[IO.File]::ReadAllText($_.FullName,[Text.Encoding]::UTF8)
 [IO.File]::WriteAllText($_.FullName,$text,(New-Object Text.UTF8Encoding($true)))
}
Set-Content (Join-Path $destination 'VERSION.txt') $Version -Encoding ASCII
Set-Content (Join-Path $destination 'Открыть_программу.cmd') '@powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\server\Open-Browser.ps1"' -Encoding ASCII
Set-Content (Join-Path $destination 'Первичная_настройка.cmd') '@powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\server\Open-Browser.ps1" -Local' -Encoding ASCII
if(-not(Test-Path (Join-Path $destination 'frontend/legacy/app.js'))){throw 'Legacy frontend missing'}
$files=@(Get-ChildItem -LiteralPath $destination -File -Recurse | ForEach-Object {@{path=$_.FullName.Substring((Resolve-Path $destination).Path.Length+1).Replace('\','/');bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}})
@{version=$Version;files=$files} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $destination 'package-manifest.json') -Encoding UTF8
$zip=Join-Path $OutputPath "ProductionCycle-server-$Version-windows-x64.zip"
Compress-Archive -LiteralPath $destination -DestinationPath $zip -Force
(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() | Set-Content ($zip+'.sha256') -Encoding ASCII
Write-Host $zip
