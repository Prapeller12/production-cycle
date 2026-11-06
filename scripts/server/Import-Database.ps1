$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
try {
 . (Join-Path $PSScriptRoot 'Common.ps1')
 if((Invoke-ProductionExe -Arguments @('--check')) -eq 0){throw 'Сначала остановите эту копию программы. Службу остановите через Службы Windows.'}
 $config=Get-Content -LiteralPath (Join-Path $script:ProductionRoot 'config/server.json') -Raw | ConvertFrom-Json
 if(Test-Path -LiteralPath (Join-Path $script:ProductionRoot ($config.dataDirectory+'/production-cycle.db'))){throw 'В этой папке уже есть база. Чтобы не перезаписать данные, перенос выполняется только в новую распакованную папку программы до её первого запуска.'}
 $dialog=New-Object Windows.Forms.OpenFileDialog
 $dialog.Title='Выберите действующую базу production-cycle.db';$dialog.Filter='База данных (*.db)|*.db';$dialog.CheckFileExists=$true
 if($dialog.ShowDialog() -ne 'OK'){exit}
 $message="Перед переносом закройте все старые клиенты и сохраните резервную копию общей папки.`r`n`r`nВыбрана база:`r`n$($dialog.FileName)`r`n`r`nИсходная база будет сохранена. Выполнить перенос?"
 if([Windows.Forms.MessageBox]::Show($message,'Перенос базы','YesNo','Question') -ne 'Yes'){exit}
 if((Invoke-ProductionExe -Arguments @('--import-database',('"'+$dialog.FileName+'"'))) -ne 0){throw 'Перенос не выполнен. Проверьте доступ к исходному файлу, целостность базы и отсутствие уже созданной базы в новой папке.'}
 [Windows.Forms.MessageBox]::Show('База перенесена. Теперь можно запускать программу или настраивать общий сервер. Используйте прежние профили и PIN.','Перенос завершён','OK','Information') | Out-Null
}catch{[Windows.Forms.MessageBox]::Show($_.Exception.Message,'Не удалось перенести базу','OK','Error') | Out-Null;exit 1}
