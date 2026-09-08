param([Parameter(Mandatory=$true)][string]$FixedRuntimePath,[string]$OutputPath='./dist/production-cycle')
$ErrorActionPreference='Stop'
$source=Split-Path -Parent $PSScriptRoot
$runtime=(Resolve-Path $FixedRuntimePath).Path
if (-not (Test-Path (Join-Path $runtime 'msedgewebview2.exe'))) {throw 'Provide the extracted Microsoft Fixed WebView2 x64 runtime; no download or installation is performed on the user computer.'}
$exe=Join-Path $source 'src-tauri/target/release/production-cycle.exe'
if (-not (Test-Path $exe)) {throw 'Build first: cargo build --release --features custom-protocol --manifest-path production-cycle/src-tauri/Cargo.toml'}
$destination=[System.IO.Path]::GetFullPath($OutputPath)
if (Test-Path $destination) {throw 'Output already exists. Choose a new output folder.'}
$root=Join-Path $destination 'ProductionCycle'
New-Item -ItemType Directory -Path (Join-Path $root 'app/backend'),(Join-Path $root 'runtime/webview2'),(Join-Path $root 'data'),(Join-Path $root 'exports'),(Join-Path $root 'temp') -Force | Out-Null
Copy-Item $exe (Join-Path $root 'app/backend/production-cycle.exe')
Copy-Item (Join-Path $source 'frontend') $root -Recurse
Copy-Item (Join-Path $source 'config') $root -Recurse
Copy-Item (Join-Path $source 'docs') $root -Recurse
Copy-Item (Join-Path $source 'README.md') $root
Copy-Item (Join-Path $runtime '*') (Join-Path $root 'runtime/webview2') -Recurse
Copy-Item (Join-Path $source 'start.cmd') $root
# The application grants the Microsoft-required read/execute ACLs inside runtime/webview2 at launch.
Compress-Archive -Path $root -DestinationPath (Join-Path $destination 'ProductionCycle-prototype-1-windows-x64.zip')
(Get-FileHash (Join-Path $destination 'ProductionCycle-prototype-1-windows-x64.zip') -Algorithm SHA256).Hash | Set-Content (Join-Path $destination 'ProductionCycle-prototype-1-windows-x64.zip.sha256') -Encoding ascii
