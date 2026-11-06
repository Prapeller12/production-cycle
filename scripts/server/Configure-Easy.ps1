param([switch]$Elevated,[string]$Address,[string]$RemoteAddresses,[int]$Port=8443,[switch]$Accept,[switch]$NoBrowser)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
try {
 . (Join-Path $PSScriptRoot 'Common.ps1')
 . (Join-Path $PSScriptRoot 'Browser.ps1')
 $identity=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
 if(-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  if($Elevated){throw 'Для настройки сервера нужны права администратора Windows.'}
  Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Verb RunAs -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$PSCommandPath+'"'),'-Elevated') -Wait
  exit
 }
 $root=$script:ProductionRoot;$exe=Get-ProductionExe
 Write-Host 'Производственный цикл — настройка общего сервера'
 Write-Host 'Выполняйте на сервере, где постоянно будут храниться данные.'
 Write-Host 'Настройка остановит текущий запуск программы. Сотрудники должны завершить сохранение.'
 if(-not $Accept -and (Read-Host 'Введите ДА для продолжения') -ne 'ДА'){exit}
 Write-Host 'IPv4-адреса этого компьютера:'
 Get-NetIPAddress -AddressFamily IPv4 | Where-Object {$_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*'} | ForEach-Object {Write-Host ('  '+$_.IPAddress+'  '+$_.InterfaceAlias)}
 if(-not $Address -and -not $Accept){$Address=Read-Host 'Введите постоянный IP или имя сервера, доступное сотрудникам'}
 $address=$Address.Trim()
 if(-not $address -or $address -match '^(localhost|127\.)' -or $address -eq '0.0.0.0'){throw 'Для сотрудников нужен сетевой IP или имя сервера. localhost здесь не подходит.'}
 if(-not $RemoteAddresses -and -not $Accept){$RemoteAddresses=Read-Host 'Разрешённые IP или подсети через запятую. Enter = только локальная подсеть'}
 $remotes=([string]$RemoteAddresses).Trim()
 if(-not $remotes){$remotes='LocalSubnet'}
 $existing=Get-CimInstance Win32_Service -Filter "Name='ProductionCycleServer'"
 if($existing -and -not $existing.PathName.StartsWith(('"'+$exe+'"'),[StringComparison]::OrdinalIgnoreCase)){throw 'Служба ProductionCycleServer уже относится к другой папке. Обновляйте существующую папку по инструкции.'}
 if($existing){Stop-Service ProductionCycleServer;(Get-Service ProductionCycleServer).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(30))}
 elseif((Invoke-ProductionExe -Arguments @('--check')) -eq 0){if((Invoke-ProductionExe -Arguments @('--stop')) -ne 0){throw 'Не удалось остановить ручной запуск. Завершите его перед настройкой.'}}
 $renew=$false;$publicCertificate=Join-Path $root 'server.cer'
 if(Test-Path -LiteralPath $publicCertificate){$oldCertificate=New-Object Security.Cryptography.X509Certificates.X509Certificate2($publicCertificate);$renew=$oldCertificate.NotAfter -lt (Get-Date).AddDays(30)}
 & (Join-Path $PSScriptRoot 'Configure-Server.ps1') -Address $address -Port $Port -RemoteAddresses $remotes -OpenFirewall -RenewCertificate:$renew
 if($existing){Set-Service ProductionCycleServer -StartupType Automatic;Start-Service ProductionCycleServer}else{& (Join-Path $PSScriptRoot 'Install-Service.ps1')}
 $deadline=[DateTime]::UtcNow.AddSeconds(30);$ready=$false
 do{if((Invoke-ProductionExe -Arguments @('--check')) -eq 0){$ready=$true;break};Start-Sleep -Milliseconds 200}while([DateTime]::UtcNow -lt $deadline)
 if(-not $ready){throw 'Служба создана, но сервер не подтвердил готовность. Проверьте службу ProductionCycleServer и доступ к папке данных.'}
 & (Join-Path $PSScriptRoot 'Export-Client.ps1')
 Import-Certificate -FilePath (Join-Path $root 'server.cer') -CertStoreLocation 'Cert:\CurrentUser\Root' | Out-Null
 $config=Get-Content -LiteralPath (Join-Path $root 'config/server.json') -Raw | ConvertFrom-Json
 Write-Host "ГОТОВО. Адрес сотрудников: $($config.publicUrl)"
 Write-Host 'Передайте сотрудникам только Подключение_сотрудника.zip.'
 Write-Host 'Первого администратора создайте в браузере на этом сервере.'
 if(-not $NoBrowser){Open-ProductionBrowser -Url "https://localhost:$Port"}
 if(-not $Accept){Read-Host 'Нажмите Enter, чтобы закрыть настройку'}
}catch{
 Write-Host $_.Exception.Message -ForegroundColor Red
 if(-not $Accept){[Windows.Forms.MessageBox]::Show($_.Exception.Message,'Настройка сервера не завершена','OK','Error') | Out-Null}
 exit 1
}
