param([Parameter(Mandatory=$true)][string]$OutputPath,[ValidateSet('portrait','landscape')][string]$Orientation='portrait',[switch]$Remove)
$ErrorActionPreference='Stop'
$name='ProductionCycle print QA '+[IO.Path]::GetFileNameWithoutExtension($OutputPath)
if($Remove){
 if(Get-Printer -Name $name -ErrorAction SilentlyContinue){Remove-Printer -Name $name}
 # Windows 2025 can retain a completed spool handle briefly after removing the queue.
 # A private, uniquely named test port is safe for the ephemeral runner to reclaim.
 if(Get-PrinterPort -Name $OutputPath -ErrorAction SilentlyContinue){Remove-PrinterPort -Name $OutputPath -ErrorAction SilentlyContinue}
 if(Test-Path "$OutputPath.default-printer.txt"){
  $original=Get-Content "$OutputPath.default-printer.txt" -Raw
  if($original.Trim() -and (Get-Printer -Name $original.Trim() -ErrorAction SilentlyContinue)){(New-Object -ComObject WScript.Network).SetDefaultPrinter($original.Trim())}
 }
 exit
}
Start-Service Spooler
$original=Get-CimInstance Win32_Printer | Where-Object Default | Select-Object -First 1 -ExpandProperty Name
Set-Content -LiteralPath "$OutputPath.default-printer.txt" -Value $original -Encoding utf8
if(-not(Get-PrinterDriver -Name 'Microsoft Print To PDF' -ErrorAction SilentlyContinue)){
 Enable-WindowsOptionalFeature -Online -FeatureName Printing-PrintToPDFServices-Features -All -NoRestart | Out-Null
 Add-PrinterDriver -Name 'Microsoft Print To PDF'
}
if(Get-Printer -Name $name -ErrorAction SilentlyContinue){Remove-Printer -Name $name}
if(-not(Get-PrinterPort -Name $OutputPath -ErrorAction SilentlyContinue)){Add-PrinterPort -Name $OutputPath}
Add-Printer -Name $name -DriverName 'Microsoft Print To PDF' -PortName $OutputPath
Add-Type -AssemblyName System.Printing,ReachFramework
$printServer=New-Object System.Printing.LocalPrintServer
$queue=New-Object System.Printing.PrintQueue($printServer,$name,[System.Printing.PrintSystemDesiredAccess]::AdministratePrinter)
$ticket=$queue.DefaultPrintTicket
$ticket.PageMediaSize=New-Object System.Printing.PageMediaSize([System.Printing.PageMediaSizeName]::ISOA4)
$ticket.PageOrientation=if($Orientation -eq 'landscape'){[System.Printing.PageOrientation]::Landscape}else{[System.Printing.PageOrientation]::Portrait}
$queue.DefaultPrintTicket=$ticket
$queue.UserPrintTicket=$ticket
$queue.Commit()
$queue.Dispose();$printServer.Dispose()
$settings='HKCU:\Software\Microsoft\Internet Explorer\PageSetup'
New-Item -Path $settings -Force | Out-Null
foreach($key in @('header','footer')){Set-ItemProperty -Path $settings -Name $key -Value ''}
foreach($key in @('margin_left','margin_right','margin_top','margin_bottom')){Set-ItemProperty -Path $settings -Name $key -Value '0.30'}
Set-ItemProperty -Path $settings -Name 'Shrink_To_Fit' -Value 'no'
$network=New-Object -ComObject WScript.Network
$network.SetDefaultPrinter($name)
Write-Host "QA printer ready: A4 $Orientation -> $OutputPath"
