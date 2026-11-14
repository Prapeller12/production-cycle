param([string]$Destination,[switch]$NoOpen)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
try{
 . (Join-Path $PSScriptRoot 'Common.ps1')
 . (Join-Path $PSScriptRoot 'SharedFolder.ps1')
 $root=$script:ProductionRoot
 $config=Get-Content -LiteralPath (Join-Path $root 'config/server.json') -Raw | ConvertFrom-Json
 $uri=[Uri]$config.publicUrl
 if(-not $config.tls -or -not $uri.IsAbsoluteUri -or $uri.Scheme -ne 'https' -or $uri.IsLoopback){throw 'Сначала выполните Настроить_сервер.cmd на сервере и укажите его постоянный сетевой IP или имя.'}
 if((Invoke-ProductionExe -Arguments @('--check') -Root $root -Quiet) -ne 0){throw 'Сервер программы не готов. Запустите службу Производственный цикл — сервер и повторите создание папки запуска.'}
 $certificate=Join-Path $root 'server.cer'
 $launcher=Join-Path $PSScriptRoot 'SharedFolderLauncher.exe'
 if(-not(Test-Path -LiteralPath $launcher)){throw 'Не найден файл кнопки запуска. Нужен полный ZIP варианта Общая папка.'}
 if(-not $Destination){
  $dialog=New-Object Windows.Forms.FolderBrowserDialog
  $dialog.Description='Выберите существующую общую папку. Внутри будет создана папка Производственный цикл только с файлами открытия.'
  if($dialog.ShowDialog() -ne 'OK'){exit}
  $Destination=Join-Path $dialog.SelectedPath 'Производственный цикл'
  $dialog.Dispose()
 }
 $Destination=[IO.Path]::GetFullPath($Destination)
 if(Test-Path -LiteralPath $Destination){
  foreach($entry in Get-ChildItem -LiteralPath $Destination -Force){
   if($entry.Name -notin @('Открыть_производственный_цикл.exe','Служебные_файлы','ПРОЧИТАЙТЕ.html')){throw 'В выбранной папке есть посторонние файлы. Выберите новую папку для кнопки запуска.'}
   if($entry.PSIsContainer){
    if($entry.Name -ne 'Служебные_файлы'){throw 'В папке запуска есть неизвестный каталог.'}
    foreach($file in Get-ChildItem -LiteralPath $entry.FullName -Force){if($file.PSIsContainer -or $file.Name -notin @('server.cer','client.json','Browser.ps1','SharedFolder.ps1','Open-SharedFolder.ps1')){throw 'В служебной папке есть посторонние файлы. Выберите новую папку.'}}
   }elseif($entry.Name -eq 'Служебные_файлы'){throw 'Имя служебной папки занято файлом.'}
  }
 }
 $support=Join-Path $Destination 'Служебные_файлы'
 New-Item -ItemType Directory -Path $support -Force | Out-Null
 $metadata=@{variant='shared-folder-browser';url=$uri.AbsoluteUri.TrimEnd('/');certificateSha256=(Get-ProductionCertificateHash -Path $certificate)}
 $metadataJson=$metadata | ConvertTo-Json
 # Validate the public certificate before publishing any client files.
 $cert=New-Object Security.Cryptography.X509Certificates.X509Certificate2($certificate)
 if($cert.HasPrivateKey -or $cert.NotAfter -lt (Get-Date) -or $cert.NotBefore -gt (Get-Date)){throw 'Публичный сертификат сервера недействителен. Повторите настройку сервера.'}
 Copy-Item -LiteralPath $launcher -Destination (Join-Path $Destination 'Открыть_производственный_цикл.exe') -Force
 foreach($name in @('Browser.ps1','SharedFolder.ps1','Open-SharedFolder.ps1')){
  $text=[IO.File]::ReadAllText((Join-Path $PSScriptRoot $name),[Text.Encoding]::UTF8)
  [IO.File]::WriteAllText((Join-Path $support $name),$text,(New-Object Text.UTF8Encoding($true)))
 }
 Copy-Item -LiteralPath $certificate -Destination (Join-Path $support 'server.cer') -Force
 [IO.File]::WriteAllText((Join-Path $support 'client.json'),$metadataJson,(New-Object Text.UTF8Encoding($false)))
 $url=[Net.WebUtility]::HtmlEncode($metadata.url);$hash=$metadata.certificateSha256
 $help=@"
<!doctype html><html lang="ru"><meta charset="utf-8"><title>Производственный цикл</title>
<style>body{font:19px/1.5 Arial,sans-serif;max-width:850px;margin:32px auto;padding:0 24px;color:#111;background:#fff}h1,h2{line-height:1.2}strong{font-weight:700}code{font-size:16px;word-break:break-all}</style>
<h1>Производственный цикл</h1><p>Откройте <strong>Открыть_производственный_цикл.exe</strong> в этой папке. Программа откроется в отдельном окне Edge. При отсутствии Edge предусмотрен установленный IE11.</p>
<p>В первый раз сверьте SHA256 сертификата с администратором лично или по телефону и подтвердите подключение. Если Windows дополнительно спросит об установке сертификата, сверьте показанный SHA1 с первым окном и подтвердите. Затем на этом пользователе Windows достаточно одного нажатия. Права администратора и копирование папки на свой ПК не требуются.</p>
<p>Адрес: <strong>$url</strong><br>SHA256: <code>$hash</code></p>
<p>Сервер и база работают на сервере организации. Интернет не нужен. Доступ к общей папке должен дополняться доступом к адресу программы. Если открытие не удалось, передайте администратору точный текст сообщения.</p>
<p>Не перемещайте кнопку отдельно от Служебные_файлы. Эту папку сотрудникам достаточно разрешить на чтение и запуск. Администратор обновляет её после изменения адреса или сертификата.</p></html>
"@
 [IO.File]::WriteAllText((Join-Path $Destination 'ПРОЧИТАЙТЕ.html'),$help,(New-Object Text.UTF8Encoding($false)))
 Write-Host "ГОТОВО. Папка открытия: $Destination"
 Write-Host "SHA256 сертификата: $hash"
 Write-Host 'Сотрудникам нужны только чтение и запуск этой папки. Права на серверную папку с базой не выдавайте.'
 if(-not $NoOpen){
  [Windows.Forms.MessageBox]::Show(("Готово. Папка запуска:`r`n"+$Destination+"`r`n`r`nСотрудники открывают в ней Открыть_производственный_цикл.exe. Серверную папку и базу не копируйте."),'Папка запуска создана','OK','Information') | Out-Null
  Start-Process explorer.exe -ArgumentList ('"'+$Destination+'"')
 }
}catch{
 Write-Host $_.Exception.Message -ForegroundColor Red
 if(-not $NoOpen){[Windows.Forms.MessageBox]::Show($_.Exception.Message,'Не удалось создать папку запуска','OK','Error') | Out-Null}
 exit 1
}
