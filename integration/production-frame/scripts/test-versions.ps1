param([string]$Output='./dist/version-tests')
$ErrorActionPreference='Stop'
$source=Split-Path -Parent $PSScriptRoot
$out=[IO.Path]::GetFullPath($Output)
if(Test-Path $out){throw 'Use a fresh test directory'}
New-Item -ItemType Directory $out | Out-Null
Copy-Item (Join-Path $source 'config') $out -Recurse
$vswhere="${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
$vs=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars=Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
$files=@('scripts/test-versions.cpp','src/versions.cpp','src/portable.cpp') | ForEach-Object {'"'+(Join-Path $source $_)+'"'}
$command='call "'+$vcvars+'" && cl /nologo /std:c++17 /W4 /WX /EHsc /MT /utf-8 /O2 '+($files -join ' ')+' /Fe:"'+$out+'/version-tests.exe" /link user32.lib gdi32.lib shell32.lib comdlg32.lib'
Push-Location $out
try {
 & cmd.exe /d /c $command
 if($LASTEXITCODE -ne 0){throw 'Version test compilation failed'}
 & ./version-tests.exe
 if($LASTEXITCODE -ne 0){throw 'Version tests failed'}
} finally {Pop-Location}
