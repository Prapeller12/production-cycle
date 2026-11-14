$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
try {
 . (Join-Path $PSScriptRoot 'Browser.ps1')
 $config=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'client.json') -Raw | ConvertFrom-Json
 $uri=[Uri]$config.url
 if($uri.Scheme -ne 'https' -or $uri.UserInfo -or $uri.Query -or $uri.Fragment -or $uri.AbsolutePath -ne '/'){throw 'Некорректный адрес сервера в комплекте подключения'}
 $path=Join-Path $PSScriptRoot 'server.cer'
 $hash=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -ne $config.certificateSha256){throw 'Сертификат отличается от указанного в комплекте. Получите комплект заново у администратора.'}
 $cert=New-Object Security.Cryptography.X509Certificates.X509Certificate2($path)
 if($cert.NotAfter -lt (Get-Date) -or $cert.NotBefore -gt (Get-Date)){throw 'Сертификат ещё не действует или истёк. Проверьте дату ПК и обратитесь к администратору.'}
 $browser=Get-ProductionBrowser
 $message="Адрес программы:`r`n$($config.url)`r`n`r`nSHA256 сертификата:`r`n$hash`r`n`r`nСверьте отпечаток с администратором по отдельному каналу. Доверие будет добавлено только для текущего пользователя Windows.`r`n`r`nПодключить программу?"
 if([Windows.Forms.MessageBox]::Show($message,'Подключение Производственного цикла','YesNo','Question') -ne 'Yes'){exit}
 Import-Certificate -FilePath $path -CertStoreLocation 'Cert:\CurrentUser\Root' | Out-Null
 $shell=New-Object -ComObject WScript.Shell
 $shortcut=$shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'Производственный цикл.lnk'))
 $shortcut.TargetPath=$browser.Path
 $shortcut.Arguments=$(if($browser.Edge){'--no-first-run --new-window '+$config.url}else{$config.url})
 $shortcut.Description='Открыть программу на сервере организации';$shortcut.Save()
 Open-ProductionBrowser -Url $config.url
}catch{[Windows.Forms.MessageBox]::Show($_.Exception.Message,'Не удалось подключить программу','OK','Error') | Out-Null;exit 1}
