param([Parameter(Mandatory=$true)][string]$Version,[Parameter(Mandatory=$true)][string]$BaseZip,[string]$OutputPath='./dist/shared-folder')
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$baseHash='c100a0c8b772415ff1d76287fdd9567ac2e152e3763518c63f1cdc04d083f466'
if((Get-FileHash -LiteralPath $BaseZip -Algorithm SHA256).Hash.ToLowerInvariant() -cne $baseHash){throw 'The base ZIP is not the verified b7dbce3 release'}
New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
$output=(Resolve-Path -LiteralPath $OutputPath).Path
$destination=Join-Path $output 'ProductionCycleServer'
if(Test-Path -LiteralPath $destination){Remove-Item -LiteralPath $destination -Recurse -Force}
Expand-Archive -LiteralPath $BaseZip -DestinationPath $output -Force
foreach($name in @('SharedFolder.ps1','Open-SharedFolder.ps1','Export-SharedFolder.ps1')){
 $text=[IO.File]::ReadAllText((Join-Path $repo "scripts/server/$name"),[Text.Encoding]::UTF8)
 [IO.File]::WriteAllText((Join-Path $destination "scripts/server/$name"),$text,(New-Object Text.UTF8Encoding($true)))
}
$compiler=Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if(-not(Test-Path -LiteralPath $compiler)){throw 'Build host is missing the Windows .NET Framework compiler'}
$clientExe=Join-Path $destination 'scripts/server/SharedFolderLauncher.exe'
& $compiler /nologo /codepage:65001 /target:winexe /platform:x64 /optimize+ /reference:System.Windows.Forms.dll ("/win32manifest:"+(Join-Path $repo 'shared-launcher/app.manifest')) ("/out:"+$clientExe) (Join-Path $repo 'shared-launcher/Program.cs')
if($LASTEXITCODE -ne 0 -or -not(Test-Path -LiteralPath $clientExe)){throw 'Shared folder launch button build failed'}
Set-Content -LiteralPath (Join-Path $destination 'Создать_папку_запуска.cmd') -Value '@powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\server\Export-SharedFolder.ps1"' -Encoding ASCII
foreach($extension in @('docx','html')){
 $guide=Join-Path $repo "dist/shared-manual/ProductionCycle_SharedFolder_Guide.$extension"
 if(-not(Test-Path -LiteralPath $guide)){throw "Shared folder guide missing: $guide"}
 Copy-Item -LiteralPath $guide -Destination $destination
}
Copy-Item -LiteralPath (Join-Path $repo 'docs/server-shared-folder.md') -Destination (Join-Path $destination 'НАЧНИТЕ_ЗДЕСЬ.md')
Set-Content -LiteralPath (Join-Path $destination 'VERSION.txt') -Value @("shared-folder-$Version","server-base=b7dbce3139e810f3624ed6d6cf175d337391d592") -Encoding ASCII
$baseManifest=Get-Content -LiteralPath (Join-Path $destination 'package-manifest.json') -Raw | ConvertFrom-Json
# The new package must preserve the exact tested executable and every frontend byte.
foreach($entry in $baseManifest.files | Where-Object {$_.path -eq 'Производственный_цикл.exe' -or $_.path.StartsWith('frontend/')} ){
 if((Get-FileHash -LiteralPath (Join-Path $destination $entry.path) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $entry.sha256){throw "Unexpected change in verified server/frontend: $($entry.path)"}
}
Remove-Item -LiteralPath (Join-Path $destination 'package-manifest.json')
$files=@(Get-ChildItem -LiteralPath $destination -File -Recurse | ForEach-Object {@{path=$_.FullName.Substring($destination.Length+1).Replace('\','/');bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}})
@{version="shared-folder-$Version";baseServer='b7dbce3';baseZipSha256=$baseHash;files=$files} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $destination 'package-manifest.json') -Encoding UTF8
$zip=Join-Path $output "ProductionCycle-shared-folder-$Version-windows-x64.zip"
Compress-Archive -LiteralPath $destination -DestinationPath $zip -Force
(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() | Set-Content -LiteralPath ($zip+'.sha256') -Encoding ASCII
Write-Host $zip
