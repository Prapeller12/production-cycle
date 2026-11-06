param([string]$Destination)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Common.ps1')
$root=$script:ProductionRoot
$config=Get-Content -LiteralPath (Join-Path $root 'config/server.json') -Raw | ConvertFrom-Json
if(-not $config.tls){throw 'Комплект сотрудников создаётся для сервера с HTTPS. Сначала выполните Настроить_сервер.cmd.'}
$certificate=Join-Path $root 'server.cer'
if(-not(Test-Path -LiteralPath $certificate)){throw 'Публичный сертификат server.cer не найден. Подготовьте сервер командой --init.'}
if(-not $Destination){$Destination=Join-Path $root 'Подключение_сотрудника'}
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
$allowed=@('server.cer','client.json','Browser.ps1','Connect-Client.ps1','Open-Client.ps1','Подключить.cmd','Открыть_программу.cmd','ПРОЧИТАЙТЕ.txt')
foreach($entry in Get-ChildItem -LiteralPath $Destination -Force){if($entry.PSIsContainer -or $entry.Name -notin $allowed){throw 'Папка комплекта содержит посторонние файлы. Используйте новую пустую папку для подключения сотрудников.'}}
Copy-Item -LiteralPath $certificate -Destination (Join-Path $Destination 'server.cer') -Force
foreach($name in @('Browser.ps1','Connect-Client.ps1','Open-Client.ps1')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $Destination -Force}
$metadata=@{url=$config.publicUrl;certificateSha256=(Get-ProductionCertificateHash -Path $certificate)}
[IO.File]::WriteAllText((Join-Path $Destination 'client.json'),($metadata|ConvertTo-Json),(New-Object Text.UTF8Encoding($false)))
Set-Content -LiteralPath (Join-Path $Destination 'Подключить.cmd') -Value '@powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Connect-Client.ps1"' -Encoding ASCII
Set-Content -LiteralPath (Join-Path $Destination 'Открыть_программу.cmd') -Value '@powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Open-Client.ps1"' -Encoding ASCII
$help="Производственный цикл`r`n`r`n1. Распакуйте комплект на локальный диск своего ПК.`r`n2. Запустите Подключить.cmd.`r`n3. Сверьте SHA256 сертификата с администратором и подтвердите подключение.`r`n4. Для ежедневной работы используйте ярлык на рабочем столе.`r`n`r`nАдрес: $($metadata.url)`r`nSHA256 сертификата: $($metadata.certificateSha256)`r`n`r`nИнтернет не нужен. Нужна корпоративная сеть или настроенный организацией канал к серверу. Этот комплект сам такой канал не создаёт.`r`nВ комплекте нет базы, PIN или закрытого ключа сервера.`r`n"
[IO.File]::WriteAllText((Join-Path $Destination 'ПРОЧИТАЙТЕ.txt'),$help,(New-Object Text.UTF8Encoding($true)))
$archive=$Destination+'.zip'
$files=@($allowed | ForEach-Object {Join-Path $Destination $_})
Compress-Archive -LiteralPath $files -DestinationPath $archive -Force
Write-Host "Комплект сотрудников: $archive"
Write-Host "SHA256 сертификата: $($metadata.certificateSha256)"
