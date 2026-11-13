param([Parameter(Mandatory=$true)][string]$FixedRuntimePath,[Parameter(Mandatory=$true)][string]$Version,[string]$OutputPath='./dist/native-share')
$ErrorActionPreference='Stop'
& "$PSScriptRoot/package-network-windows.ps1" -FixedRuntimePath $FixedRuntimePath -Version $Version -OutputPath $OutputPath
$root=Join-Path ([IO.Path]::GetFullPath($OutputPath)) 'ProductionCycleNetwork'
Remove-Item "$root/frontend/legacy" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$root/docs","$root/README.md" -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path "$root/samples" -Force | Out-Null
$sample=Get-Content "$PSScriptRoot/../server/resources/project-import-template.json" -Raw | ConvertFrom-Json
[IO.File]::WriteAllBytes((Join-Path "$root/samples" $sample.fileName),[Convert]::FromBase64String($sample.base64))
[IO.File]::WriteAllText("$root/КАК ЗАПУСТИТЬ.txt",[IO.File]::ReadAllText("$PSScriptRoot/../docs/native-share-startup.txt"),[Text.UTF8Encoding]::new($true))
# Refresh the immutable executable payload after selecting the native UI.
$entries=@(foreach($name in @('production-cycle.exe','frontend','runtime','config','app')){
 $item=Get-Item (Join-Path $root $name)
 $files=if($item.PSIsContainer){Get-ChildItem $item.FullName -File -Recurse}else{@($item)}
 foreach($file in $files){@{path=[IO.Path]::GetRelativePath($root,$file.FullName).Replace('\','/');bytes=$file.Length;sha256=(Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}}
})
$manifest=@{format='production-cycle-launch-v1';version=$Version;files=@($entries|Sort-Object path)}|ConvertTo-Json -Depth 6
[IO.File]::WriteAllText("$root/launch-manifest.json",$manifest,[Text.UTF8Encoding]::new($false))
Remove-Item "$OutputPath/*.zip","$OutputPath/*.sha256" -Force
Compress-Archive -LiteralPath $root -DestinationPath "$OutputPath/ProductionCycle-native-share-windows-x64.zip" -CompressionLevel Optimal
(Get-FileHash "$OutputPath/ProductionCycle-native-share-windows-x64.zip" -Algorithm SHA256).Hash.ToLowerInvariant()|Set-Content "$OutputPath/ProductionCycle-native-share-windows-x64.zip.sha256" -Encoding ascii
