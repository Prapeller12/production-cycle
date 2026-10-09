param(
  [Parameter(Mandatory=$true)][string]$FixedRuntimePath,
  [Parameter(Mandatory=$true)][string]$Version,
  [string]$OutputPath='./dist/network'
)
$ErrorActionPreference='Stop'
$source=Split-Path -Parent $PSScriptRoot
$runtime=(Resolve-Path $FixedRuntimePath).Path
if (-not (Test-Path (Join-Path $runtime 'msedgewebview2.exe'))) {throw 'Provide the extracted Microsoft Fixed WebView2 x64 runtime.'}
$exe=Join-Path $source 'src-tauri/target/release/production-cycle.exe'
if (-not (Test-Path $exe)) {throw 'Build the release host before packaging.'}
$destination=[System.IO.Path]::GetFullPath($OutputPath)
if (Test-Path $destination) {throw 'Output already exists. Choose a new output folder.'}
$root=Join-Path $destination 'ProductionCycleNetwork'
New-Item -ItemType Directory -Path `
  (Join-Path $root 'app'),(Join-Path $root 'runtime/webview2'),(Join-Path $root 'config'),`
  (Join-Path $root 'shared/database'),(Join-Path $root 'shared/sessions'),(Join-Path $root 'shared/control'),`
  (Join-Path $root 'shared/drafts'),(Join-Path $root 'shared/backups'),(Join-Path $root 'shared/exports') -Force | Out-Null
Copy-Item $exe (Join-Path $root 'production-cycle.exe')
Copy-Item (Join-Path $source 'frontend') $root -Recurse
Copy-Item (Join-Path $runtime '*') (Join-Path $root 'runtime/webview2') -Recurse
Copy-Item (Join-Path $source 'config/portable.json') (Join-Path $root 'config/portable.json')
$network=Get-Content (Join-Path $source 'config/network.test.json') -Raw
$network=$network.Replace('NETWORK_VERSION',$Version)
[IO.File]::WriteAllText((Join-Path $root 'config/network.json'),$network,(New-Object Text.UTF8Encoding($false)))
Copy-Item (Join-Path $source 'docs') $root -Recurse
Copy-Item (Join-Path $source 'README.md') $root
Copy-Item (Join-Path $source 'start.cmd') (Join-Path $root 'app/start.cmd')
$readme=@"
СЕТЕВАЯ ТЕСТОВАЯ ВЕРСИЯ

1. Полностью распакуйте эту папку в общую сетевую папку с правами чтения и записи для участников теста.
2. На каждом компьютере запускайте production-cycle.exe прямо из общей папки.
3. Программа сама копирует исполняемую часть и WebView2 в локальный профиль Windows. Общие проекты остаются в папке shared.
4. Одновременно допускаются три компьютера. Четвёртый обычный пользователь не войдёт. Администратор может занять место последнего вошедшего обычного пользователя после автосохранения его черновика.
5. Не переносите один только EXE: рядом нужны frontend, runtime, config и app.

Это отдельная версия для тестовой работы. Перед началом испытаний сделайте резервную копию всей папки ProductionCycleNetwork.
"@
[IO.File]::WriteAllText((Join-Path $root 'КАК ЗАПУСТИТЬ.txt'),$readme,(New-Object Text.UTF8Encoding($true)))
$zip=Join-Path $destination 'ProductionCycle-network-test-windows10-11-x64.zip'
Compress-Archive -Path $root -DestinationPath $zip
(Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant() | Set-Content "$zip.sha256" -Encoding ascii
