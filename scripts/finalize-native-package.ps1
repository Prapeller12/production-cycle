param([Parameter(Mandatory=$true)][string]$BaseZip,[string]$OutputPath='./dist/final')
$ErrorActionPreference='Stop'
$expected='3158335ffa3eb1522e5e47993200c1166db33796f86255a91361bf35152967b3'
if((Get-FileHash -LiteralPath $BaseZip -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expected){throw 'Base ZIP differs from the verified release'}
$out=[IO.Path]::GetFullPath($OutputPath)
if(Test-Path -LiteralPath $out){throw 'Final output folder already exists'}
New-Item -ItemType Directory -Path $out|Out-Null
Expand-Archive -LiteralPath $BaseZip -DestinationPath $out
$root=Join-Path $out 'ProductionCycleNetwork';$manifestPath=Join-Path $root 'launch-manifest.json'
$manifest=Get-Content -LiteralPath $manifestPath -Raw|ConvertFrom-Json
if($manifest.version -cne 'native-startup-0c7663b'){throw 'Wrong native base version'}
$entry=@($manifest.files|Where-Object {$_.path -ceq 'app/start.cmd'})
if($entry.Count -ne 1){throw 'Native launcher is missing from the immutable payload'}
Copy-Item -LiteralPath "$PSScriptRoot/../start.cmd" -Destination "$root/app/start.cmd" -Force
[IO.File]::WriteAllText("$root/КАК ЗАПУСТИТЬ.txt",[IO.File]::ReadAllText("$PSScriptRoot/../docs/native-share-startup.txt"),[Text.UTF8Encoding]::new($true))
$changed=Get-Item -LiteralPath "$root/app/start.cmd"
$entry[0].bytes=$changed.Length;$entry[0].sha256=(Get-FileHash -LiteralPath $changed.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$verified=0
foreach($file in $manifest.files){
 $path=Join-Path $root $file.path
 if((Get-Item -LiteralPath $path).Length -ne $file.bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $file.sha256){throw "Payload changed unexpectedly: $($file.path)"}
 $verified++
}
[IO.File]::WriteAllText($manifestPath,($manifest|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
$zip=Join-Path $out 'ProductionCycle_FINAL_2026-11-13.zip'
Compress-Archive -LiteralPath $root -DestinationPath $zip -CompressionLevel Optimal
$sha=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
$sha|Set-Content ($zip+'.sha256') -Encoding ascii
New-Item -ItemType Directory -Path ./dist/final-evidence -Force|Out-Null
@{baseSHA256=$expected;finalSHA256=$sha;nativeSource='0c7663bf101322dfac8b9abcdd3a3b7b87a09f47';packageSource=$env:GITHUB_SHA;verifiedFiles=$verified;changedExecutablePayload=@('app/start.cmd');runtimeAndExePreserved=$true}|ConvertTo-Json -Depth 4|Set-Content ./dist/final-evidence/package-verification.json -Encoding utf8
