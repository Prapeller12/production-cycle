$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
try {
 . (Join-Path $PSScriptRoot 'Common.ps1')
 $answer=[Windows.Forms.MessageBox]::Show('Завершите сохранение и закройте вкладки программы. Остановить локальный сервер?','Производственный цикл','YesNo','Question')
 if($answer -ne 'Yes'){exit}
 if((Invoke-ProductionExe -Arguments @('--stop')) -ne 0){throw 'Не удалось остановить эту копию. Если программа установлена как служба, остановите ProductionCycleServer через Службы Windows с правами администратора.'}
 [Windows.Forms.MessageBox]::Show('Локальный сервер остановлен. Данные сохранены в папке программы.','Производственный цикл','OK','Information') | Out-Null
}catch{[Windows.Forms.MessageBox]::Show($_.Exception.Message,'Производственный цикл','OK','Error') | Out-Null;exit 1}
