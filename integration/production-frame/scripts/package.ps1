param(
    [Parameter(Mandatory=$true)][string]$Frame,
    [Parameter(Mandatory=$true)][string]$ReportingZip,
    [Parameter(Mandatory=$true)][string]$CycleZip,
    [string]$Output='./dist/combined'
)
$ErrorActionPreference='Stop'
$out=[IO.Path]::GetFullPath($Output)
if(Test-Path $out){throw 'Choose a new output folder; existing files are never overwritten'}
$root=Join-Path $out 'ProductionFrame'
New-Item -ItemType Directory -Path (Join-Path $root 'modules') -Force | Out-Null
Copy-Item (Join-Path $Frame '*') $root -Recurse
if(Test-Path (Join-Path $root 'temp')){Remove-Item (Join-Path $root 'temp') -Recurse}
Expand-Archive $ReportingZip (Join-Path $root 'modules')
Expand-Archive $CycleZip (Join-Path $root 'modules')
$required=@('ProductionFrame.exe','config/frame.ini',
 'modules/ReportingSystem/ReportingSystem.exe','modules/ReportingSystem/runtime/webview2/msedgewebview2.exe',
 'modules/ProductionCycle/production-cycle.exe','modules/ProductionCycle/runtime/webview2/msedgewebview2.exe')
foreach($path in $required){if(-not(Test-Path (Join-Path $root $path))){throw "Missing portable component: $path"}}
# Packaging only accepts clean upstream builds, never a user's working application folder.
foreach($module in @('ReportingSystem','ProductionCycle')) {
 foreach($dir in @('data','temp','backups','exports','imports/inbox','attachments')) {
  $path=Join-Path $root "modules/$module/$dir"
  if((Test-Path $path) -and (Get-ChildItem $path -File -Recurse | Where-Object {$_.Name -notin @('.gitkeep','.keep') })){throw "Nonempty user-data directory in build input: $path"}
 }
}
$manifest=[ordered]@{
 version='0.1.0-preview'; status='integration-preview';
 reporting=[ordered]@{repository='Prapeller12/otchet';commit='75e8c38d863f667ed64ce85b4bfb314c6126cf92';version='0.1.0-dev.20';sha256=(Get-FileHash $ReportingZip -Algorithm SHA256).Hash};
 cycle=[ordered]@{repository='Prapeller12/production-cycle';commit='2029af108a7de7636a620f01439a05d2c33a2e09';workflow_run=34493342361;sha256=(Get-FileHash $CycleZip -Algorithm SHA256).Hash};
 frame=[ordered]@{commit=$env:GITHUB_SHA;sha256=(Get-FileHash (Join-Path $root 'ProductionFrame.exe') -Algorithm SHA256).Hash};
 acceptance='Clean Windows 10/11 manual acceptance remains OPEN. See docs/verification.md.'
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $root 'manifest.json') -Encoding utf8
Compress-Archive -Path $root -DestinationPath (Join-Path $out 'ProductionFrame-0.1-windows-x64-preview.zip')
