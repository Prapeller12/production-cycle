$ErrorActionPreference='Stop'
$root=Join-Path $env:RUNNER_TEMP 'pc-exchange-bundle/ProductionCycleNetwork'
# Run after creating the ZIP, so disposable profiles and test data cannot enter the release.
$qa=Join-Path $env:RUNNER_TEMP 'pc-exchange-ui-results'
New-Item $qa -ItemType Directory -Force | Out-Null
npm ci --ignore-scripts --no-audit --no-fund
if ($LASTEXITCODE) {throw 'UI test dependencies failed'}
node tests/db-exchange-window.cjs $root offline $qa
if ($LASTEXITCODE) {throw 'Offline native UI failed'}
$sourceBackup=Get-ChildItem -LiteralPath "$root/data/backups" -File -Filter 'production-cycle_*.db' | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if(!$sourceBackup){throw 'Signed save did not produce an encrypted backup'}
& "$root/archive-encrypted-backup.ps1" -BackupFile $sourceBackup.FullName -Destination $qa
$written=Get-ChildItem -LiteralPath $qa -Directory -Filter 'ProductionCycle_*' | Select-Object -First 1
if(!$written -or (Get-FileHash -LiteralPath (Join-Path $written.FullName $sourceBackup.Name)).Hash -ne (Get-FileHash -LiteralPath $sourceBackup.FullName).Hash){throw 'Offline backup verification failed'}
Move-Item "$root/data" "$qa/offline-data"
$shareName='PCExchange'+$env:GITHUB_RUN_ID
$account=[Security.Principal.WindowsIdentity]::GetCurrent().Name
try {
 New-SmbShare -Name $shareName -Path $root -FullAccess $account | Out-Null
 $unc="\\localhost\$shareName"
 node tests/db-exchange-window.cjs $unc shared $qa
 if ($LASTEXITCODE) {throw 'SMB import UI failed'}
} finally {Remove-SmbShare -Name $shareName -Force -ErrorAction SilentlyContinue}
