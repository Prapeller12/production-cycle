$ErrorActionPreference='Stop'
$identity=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Запустите PowerShell от администратора на сервере'}
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$exe=Join-Path $root 'production-cycle-server.exe'
if(Get-Service ProductionCycleServer -ErrorAction SilentlyContinue){throw 'Служба уже существует. Для обновления остановите её, замените файлы приложения и запустите снова. Данные и TLS сохраняются.'}
& $exe --init
if($LASTEXITCODE -ne 0){throw 'Сначала исправьте конфигурацию сервера'}
$config=Get-Content (Join-Path $root 'config/server.json') -Raw | ConvertFrom-Json
foreach($relative in @($config.dataDirectory,(Split-Path $config.privateKey -Parent))){
 $directory=Join-Path $root $relative;New-Item -ItemType Directory -Path $directory -Force | Out-Null
 $acl=Get-Acl -LiteralPath $directory;$acl.SetAccessRuleProtection($true,$false)
 foreach($sid in @('S-1-5-18','S-1-5-32-544','S-1-5-19')){
  $rule=New-Object Security.AccessControl.FileSystemAccessRule([Security.Principal.SecurityIdentifier]$sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow');$acl.AddAccessRule($rule)
 }
 Set-Acl -LiteralPath $directory -AclObject $acl
}
# LocalService needs only read/execute on application files, and owns its local data/TLS.
& "$env:SystemRoot\System32\icacls.exe" $root /grant '*S-1-5-19:(OI)(CI)(RX)' /Q | Out-Null
if($LASTEXITCODE -ne 0){throw 'Не удалось выдать службе чтение серверных файлов'}
$credential=New-Object System.Management.Automation.PSCredential('NT AUTHORITY\LocalService',(New-Object Security.SecureString))
New-Service -Name ProductionCycleServer -DisplayName 'Производственный цикл — сервер' -BinaryPathName ('"'+$exe+'" --service') -StartupType Automatic -Credential $credential | Out-Null
& "$env:SystemRoot\System32\sc.exe" failure ProductionCycleServer reset= 86400 actions= restart/5000/restart/15000 | Out-Null
Start-Service ProductionCycleServer
(Get-Service ProductionCycleServer).WaitForStatus('Running',[TimeSpan]::FromSeconds(30))
Write-Host 'Служба установлена. Проверьте открытие интерфейса на сервере и с отдельного клиентского ПК.'
