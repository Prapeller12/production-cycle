param([string]$BackupFile,[string]$Destination)
$ErrorActionPreference='Stop'
$root=$PSScriptRoot
$folders=@('shared/backups','data/backups') | ForEach-Object {Join-Path $root $_} | Where-Object {Test-Path -LiteralPath $_ -PathType Container}
$available=@(foreach($folder in $folders){Get-ChildItem -LiteralPath $folder -File -Filter 'production-cycle_*.db'})
if(!$available){throw 'Сначала создайте резервную копию в разделе администрирования программы.'}
Add-Type -AssemblyName System.Windows.Forms
if(!$BackupFile){
 $dialog=New-Object Windows.Forms.OpenFileDialog
 $dialog.Title='Выберите проверенную программой резервную копию базы'
 $dialog.InitialDirectory=($available | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1).DirectoryName
 $dialog.Filter='Резервная копия программы|production-cycle_*.db'
 try{if($dialog.ShowDialog() -ne 'OK'){return};$BackupFile=$dialog.FileName}finally{$dialog.Dispose()}
}
$source=(Get-Item -LiteralPath $BackupFile).FullName
if($available.FullName -notcontains $source){throw 'Выберите штатную копию из shared/backups или data/backups этого комплекта.'}
if(!$Destination){
 $dialog=New-Object Windows.Forms.FolderBrowserDialog
 $dialog.Description='Выберите каталог на отключаемом диске или в отдельном архивном хранилище.'
 try{if($dialog.ShowDialog() -ne 'OK'){return};$Destination=$dialog.SelectedPath}finally{$dialog.Dispose()}
}
$folder=(Get-Item -LiteralPath $Destination).FullName
if(!(Test-Path -LiteralPath $folder -PathType Container)){throw 'Выберите папку архивного хранилища.'}
$prefix=$root.TrimEnd('\')+'\'
if(($folder.TrimEnd('\')+'\').StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)){throw 'Выберите хранилище вне рабочей папки программы.'}
$set=Join-Path $folder ('ProductionCycle_'+[DateTime]::UtcNow.ToString('yyyyMMdd_HHmmss')+'_'+[Guid]::NewGuid().ToString('N'))
$created=$false
$inputStream=[IO.File]::Open($source,'Open','Read','Read')
try{
 $header=New-Object byte[] 16
 if($inputStream.Read($header,0,16) -ne 16 -or [Text.Encoding]::ASCII.GetString($header).StartsWith('SQLite format 3')){throw 'Исходная копия не зашифрована или неполна.'}
 $inputStream.Position=0
 $null=New-Item -ItemType Directory -Path $set -ErrorAction Stop
 $created=$true
 $name=[IO.Path]::GetFileName($source)
 $dest=Join-Path $set $name
 $output=[IO.File]::Open($dest,'CreateNew','Write','None')
 try{$inputStream.CopyTo($output);$output.Flush($true)}finally{$output.Dispose()}
 $inputStream.Position=0
 $digest=(Get-FileHash -InputStream $inputStream -Algorithm SHA256).Hash
 if((Get-FileHash -LiteralPath $dest -Algorithm SHA256).Hash -cne $digest){throw 'Контрольные суммы копии и исходника различаются.'}
 [IO.File]::WriteAllText((Join-Path $set 'SHA256.txt'),($digest+'  '+$name),[Text.Encoding]::UTF8)
 Write-Output "Архив проверен: $set"
 Write-Output 'После записи отключите носитель или закройте доступ на запись к этой копии.'
} catch {
 if($created){foreach($name in @('SHA256.txt',[IO.Path]::GetFileName($source))){$file=Join-Path $set $name;if(Test-Path -LiteralPath $file){[IO.File]::Delete($file)}};[IO.Directory]::Delete($set)}
 throw
} finally {$inputStream.Dispose()}
