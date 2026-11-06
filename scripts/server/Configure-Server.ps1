param([string]$Address,[int]$Port=8443,[string]$RemoteAddresses='LocalSubnet',[switch]$OpenFirewall,[switch]$RenewCertificate)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Common.ps1')
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
if(-not $Address){$Address=Read-Host 'IP или имя сервера, доступное сотрудникам (например 10.10.208.24)'}
if($Address -notmatch '^[A-Za-z0-9.-]+$' -or $Port -lt 1 -or $Port -gt 65535){throw 'Некорректный адрес или порт'}
if(Get-Service ProductionCycleServer -ErrorAction SilentlyContinue | Where-Object Status -eq Running){throw 'Сначала остановите службу ProductionCycleServer'}
$file=Join-Path $root 'config/server.json';$config=Get-Content -LiteralPath $file -Raw | ConvertFrom-Json
$newUrl="https://${Address}:$Port"
if(($config.publicUrl -ne $newUrl -or $RenewCertificate) -and (Test-Path (Join-Path $root $config.certificate))){
 # Keep the previous keys intact when switching from a local test or another IP.
 $suffix=[Guid]::NewGuid().ToString('N');$config.certificate="tls/server-$suffix.pem";$config.privateKey="tls/server-$suffix-key.pem"
}
$config.listen="0.0.0.0:$Port";$config.publicUrl=$newUrl;$config.tls=$true;$config.allowInsecureLan=$false
[IO.File]::WriteAllText($file,($config | ConvertTo-Json -Depth 5),(New-Object Text.UTF8Encoding($false)))
$exe=Get-ProductionExe -Root $root
if((Invoke-ProductionExe -Arguments @('--init')) -ne 0){throw 'Не удалось подготовить сервер. Проверьте config/server.json и доступ к папке.'}
Set-Content -LiteralPath (Join-Path $root 'Открыть_программу.url') -Value "[InternetShortcut]`r`nURL=$newUrl`r`n" -Encoding ASCII
if($OpenFirewall){
 $identity=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
 if(-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Для настройки брандмауэра запустите PowerShell от администратора'}
 $name='ProductionCycleServer-HTTPS'
 if(Get-NetFirewallRule -Name $name -ErrorAction SilentlyContinue){Remove-NetFirewallRule -Name $name}
 New-NetFirewallRule -Name $name -DisplayName 'Производственный цикл — HTTPS' -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Port -Program $exe -RemoteAddress ($RemoteAddresses -split ',' | ForEach-Object {$_.Trim()}) -Profile Domain,Private | Out-Null
}
Write-Host "Адрес сотрудников: $newUrl"
Write-Host "Первого администратора создайте на самом сервере по адресу https://localhost:$Port"
Write-Host 'Публичный сертификат: server.cer. Закрытый ключ из tls на клиентские ПК не передавать.'
