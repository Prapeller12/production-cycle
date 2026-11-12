param([Parameter(Mandatory=$true)][string]$FixedRuntimePath,[string]$OutputPath='./dist/local')
$ErrorActionPreference='Stop'
$source=Split-Path -Parent $PSScriptRoot
$runtime=(Resolve-Path $FixedRuntimePath).Path
if(-not(Test-Path "$runtime/msedgewebview2.exe")){throw 'Fixed WebView2 runtime is missing'}
$destination=[IO.Path]::GetFullPath($OutputPath)
if(Test-Path $destination){throw 'Choose a new output directory'}
$root=Join-Path $destination 'ProductionCycle'
New-Item -ItemType Directory -Force -Path "$root/runtime/webview2","$root/config","$root/data","$root/exports","$root/temp","$root/app","$root/samples" | Out-Null
Copy-Item "$source/src-tauri/target/release/production-cycle.exe" "$root/production-cycle.exe"
Copy-Item "$source/frontend" $root -Recurse
# The local program only needs the modern interface. Never ship server or SMB configuration.
Remove-Item "$root/frontend/legacy" -Recurse -Force -ErrorAction SilentlyContinue
Copy-Item "$source/config/portable.json" "$root/config/portable.json"
Copy-Item "$runtime/*" "$root/runtime/webview2" -Recurse
Copy-Item "$source/docs/local/Инструкция.txt" "$root/Инструкция.txt"
$sample=Get-Content "$source/server/resources/project-import-template.json" -Raw | ConvertFrom-Json
[IO.File]::WriteAllBytes((Join-Path "$root/samples" $sample.fileName),[Convert]::FromBase64String($sample.base64))
[IO.File]::WriteAllText("$root/app/start.cmd","@echo off`r`nstart `"`" `"%~dp0..\production-cycle.exe`"`r`n",[Text.Encoding]::ASCII)
$manifest=@{format='production-cycle-local-package-v1';architecture='local-exe';commit=$env:GITHUB_SHA;runtime=(Get-Content "$source/config/webview-runtime.json" -Raw | ConvertFrom-Json).version;files=@()}
Get-ChildItem $root -Recurse -File | Sort-Object FullName | ForEach-Object {$manifest.files+=@{path=[IO.Path]::GetRelativePath($root,$_.FullName).Replace('\','/');sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant();bytes=$_.Length}}
$manifest | ConvertTo-Json -Depth 6 | Set-Content "$root/package-manifest.json" -Encoding utf8
$zip=Join-Path $destination 'ProductionCycle-local-windows-x64.zip'
Compress-Archive -LiteralPath $root -DestinationPath $zip -CompressionLevel Optimal
(Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant() | Set-Content "$zip.sha256" -Encoding ascii
