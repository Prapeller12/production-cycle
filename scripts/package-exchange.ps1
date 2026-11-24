$ErrorActionPreference='Stop'
$dist=Join-Path $env:GITHUB_WORKSPACE 'dist/exchange'
New-Item -ItemType Directory -Force $dist | Out-Null
./scripts/fetch-runtime.ps1 -Destination "$env:RUNNER_TEMP/secure-fixed-webview"
$runtime=(Get-Content "$env:RUNNER_TEMP/secure-fixed-webview/runtime-path.txt" -Raw).Trim()
$work=Join-Path $env:RUNNER_TEMP 'pc-exchange-bundle'
./scripts/package-native-share.ps1 -FixedRuntimePath $runtime -Version ('exchange-'+$env:GITHUB_SHA.Substring(0,12)) -OutputPath $work
$root=Join-Path $work 'ProductionCycleNetwork'
Copy-Item ./docs/database-encryption.json "$root/database-encryption.json"
Copy-Item ./docs/db-exchange-start.txt "$root/КАК ЗАПУСТИТЬ.txt" -Force
Remove-Item "$root/samples" -Recurse -Force -ErrorAction SilentlyContinue
[IO.File]::WriteAllText("$root/OFFLINE.cmd","@echo off`r`nstart `"`" `"%~dp0production-cycle.exe`" --offline-workstation`r`n",[Text.Encoding]::ASCII)
[IO.File]::WriteAllText("$root/BUILD.txt",$env:GITHUB_SHA,[Text.Encoding]::ASCII)
$zip=Join-Path $dist 'ProductionCycle_Windows_x64_DB_Exchange.zip'
Compress-Archive -Path "$root/*" -DestinationPath $zip -CompressionLevel Optimal
Add-Type -AssemblyName System.IO.Compression.FileSystem
$z=[IO.Compression.ZipFile]::OpenRead($zip)
try{foreach($required in @('production-cycle.exe','OFFLINE.cmd','frontend/ui/db-exchange.js','database-encryption.json','runtime/webview2/msedgewebview2.exe','launch-manifest.json','BUILD.txt')){if(!$z.GetEntry($required)){throw "Missing archive member: $required"}}}finally{$z.Dispose()}
(Get-FileHash $zip -Algorithm SHA256).Hash
