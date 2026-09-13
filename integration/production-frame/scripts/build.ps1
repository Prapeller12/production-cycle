param([string]$Output = './dist/frame')
$ErrorActionPreference = 'Stop'
$source = Split-Path -Parent $PSScriptRoot
$out = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force $out | Out-Null
# Developer build only. No compiler or development dependencies go to user machines.
$vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw 'MSVC x64 build tools not found on the build machine' }
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
$files = @('main.cpp','module.cpp','portable.cpp','versions.cpp') | ForEach-Object { '"' + (Join-Path $source "src/$_") + '"' }
$command = 'call "' + $vcvars + '" && cl /nologo /std:c++17 /W4 /WX /EHsc /MT /utf-8 /O2 ' + ($files -join ' ') + ' /Fe:"' + $out + '/ProductionFrame.exe" /link /SUBSYSTEM:WINDOWS user32.lib gdi32.lib shell32.lib comdlg32.lib'
Push-Location $out
try { & cmd.exe /d /c $command; if ($LASTEXITCODE -ne 0) { throw 'Frame compilation failed' } }
finally { Pop-Location }
Copy-Item (Join-Path $source 'config') $out -Recurse -Force
Copy-Item (Join-Path $source 'docs') $out -Recurse -Force
Copy-Item (Join-Path $source 'README.md') $out -Force
Get-ChildItem $out -Filter '*.obj' | Remove-Item
