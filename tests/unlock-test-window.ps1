# Only operates on the named dialog of this test process or its local client.
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
$until=(Get-Date).AddMinutes(2)
while((Get-Date) -lt $until){
 $windows=[Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)
 foreach($w in $windows){
  if($w.Current.Name -ne 'Производственный цикл — ключ базы'){continue}
  $owner=Get-CimInstance Win32_Process -Filter ("ProcessId="+$w.Current.ProcessId)
  if(!$owner -or $owner.Name -ne 'powershell.exe'){continue}
  $parent=Get-CimInstance Win32_Process -Filter ("ProcessId="+$owner.ParentProcessId)
  if(!$parent -or $parent.Name -ne 'production-cycle.exe'){continue}
  $edit=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Edit)
  $boxes=$w.FindAll([Windows.Automation.TreeScope]::Descendants,$edit)
  if($boxes.Count -lt 2){continue}
  $phrase=$env:PC_TEST_WINDOW_PHRASE
  foreach($box in $boxes){$box.SetFocus();[Windows.Forms.SendKeys]::SendWait($phrase)}
  [Windows.Forms.SendKeys]::SendWait('{ENTER}');exit 0
 }
 Start-Sleep -Milliseconds 250
}
throw 'Test key dialog did not appear'
