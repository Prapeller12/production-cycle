param([Parameter(Mandatory=$true)][string]$Destination)
$ErrorActionPreference='Stop'
$c=Get-Content (Join-Path $PSScriptRoot '../config/webview-runtime.json') -Raw | ConvertFrom-Json
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
$Destination=(Resolve-Path $Destination).Path
$cab=Join-Path $Destination 'runtime.cab'
& curl.exe --fail --location --retry 5 --output $cab $c.url
if($LASTEXITCODE -ne 0){throw 'Runtime download failed'}
if((Get-Item $cab).Length -ne $c.cabinetBytes){throw 'Incomplete runtime CAB. Expected cabinet size differs from downloaded bytes.'}
if((Get-FileHash $cab -Algorithm SHA256).Hash.ToLowerInvariant() -ne $c.sha256){throw 'Runtime SHA-256 mismatch'}
$unpack=Join-Path $Destination 'unpacked'
New-Item -ItemType Directory -Path $unpack -Force | Out-Null
& expand.exe $cab '-F:*' $unpack | Out-Null
if($LASTEXITCODE -ne 0){throw 'Runtime CAB extraction failed'}
$exe=@(Get-ChildItem $unpack -Filter msedgewebview2.exe -Recurse)
if($exe.Count -ne 1){throw 'Expected one Fixed Runtime executable'}
$signature=Get-AuthenticodeSignature $exe[0].FullName
if($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation'){throw 'Invalid Microsoft runtime signature'}
if($exe[0].VersionInfo.FileVersion -ne $c.version){throw 'Unexpected runtime version'}
@{version=$c.version;architecture=$c.architecture;source=$c.url;cabinetSHA256=(Get-FileHash $cab -Algorithm SHA256).Hash;cabinetBytes=(Get-Item $cab).Length;signatureStatus=$signature.Status.ToString();signer=$signature.SignerCertificate.Subject} | ConvertTo-Json | Set-Content (Join-Path $exe[0].DirectoryName 'RUNTIME-PROVENANCE.json') -Encoding utf8
$exe[0].DirectoryName | Set-Content (Join-Path $Destination 'runtime-path.txt') -Encoding utf8
