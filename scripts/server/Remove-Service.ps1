$ErrorActionPreference='Stop'
if(Get-Service ProductionCycleServer -ErrorAction SilentlyContinue){Stop-Service ProductionCycleServer;(Get-Service ProductionCycleServer).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(30));& "$env:SystemRoot\System32\sc.exe" delete ProductionCycleServer}
Write-Host 'Служба удалена. База, резервные копии, сертификат и настройки сохранены.'
