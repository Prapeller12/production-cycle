$ErrorActionPreference='Stop'
try {
 . (Join-Path $PSScriptRoot 'Browser.ps1')
 $config=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'client.json') -Raw | ConvertFrom-Json
 Open-ProductionBrowser -Url $config.url
}catch{Add-Type -AssemblyName System.Windows.Forms;[Windows.Forms.MessageBox]::Show($_.Exception.Message,'Не удалось открыть программу','OK','Error') | Out-Null;exit 1}
