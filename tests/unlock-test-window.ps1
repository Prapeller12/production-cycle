# Exercise the same native edit controls and OK action as a user on a disposable runner.
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class VaultWindowTest {
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string type, string title);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr parent, int id);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr hwnd, uint message, IntPtr param, string text);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr param, IntPtr extra);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
}
'@
$until=(Get-Date).AddSeconds(105)
while((Get-Date) -lt $until){
 $window=[VaultWindowTest]::FindWindow('ProductionCycleVaultUnlock',$null)
 if($window -ne [IntPtr]::Zero){
  [uint32]$processNumber=0
  $null=[VaultWindowTest]::GetWindowThreadProcessId($window,[ref]$processNumber)
  if((Get-Process -Id $processNumber).ProcessName -ne 'production-cycle'){throw 'Unexpected unlock window owner'}
  $first=[VaultWindowTest]::GetDlgItem($window,1101)
  $second=[VaultWindowTest]::GetDlgItem($window,1102)
  if($first -ne [IntPtr]::Zero -and $second -ne [IntPtr]::Zero){
   foreach($field in @($first,$second)){$null=[VaultWindowTest]::SendMessage($field,12,[IntPtr]::Zero,$env:PC_TEST_WINDOW_PHRASE)}
   $button=[VaultWindowTest]::GetDlgItem($window,1)
   if(![VaultWindowTest]::PostMessage($button,245,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Could not press unlock button'}
   exit 0
  }
 }
 Start-Sleep -Milliseconds 250
}
throw 'Test key dialog did not appear'
