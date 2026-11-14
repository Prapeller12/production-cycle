$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
try{
 . (Join-Path $PSScriptRoot 'Browser.ps1')
 . (Join-Path $PSScriptRoot 'SharedFolder.ps1')
 $connection=Get-ProductionSharedConnection -Directory $PSScriptRoot
 $null=Get-ProductionBrowser
 if(-not(Confirm-ProductionSharedTrust -Connection $connection)){exit 2}
 Test-ProductionSharedServer -Connection $connection
 Open-ProductionBrowser -Url $connection.Url
}catch{
 [Windows.Forms.MessageBox]::Show($_.Exception.Message,'Не удалось открыть производственный цикл','OK','Error') | Out-Null
 exit 1
}
