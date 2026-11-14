param([Parameter(Mandatory=$true)][string]$Zip)
$ErrorActionPreference='Stop'
$work=Join-Path $env:RUNNER_TEMP ('SharedFolderAcceptance-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $work
$root=Join-Path $work 'ProductionCycleServer'
$public=Join-Path $work 'Общая папка с пробелами & кириллицей'
$destination=Join-Path $public 'Производственный цикл'
$shareName='ProductionCycle'+[Guid]::NewGuid().ToString('N').Substring(0,10)
$unc='\\localhost\'+$shareName+'\Производственный цикл'
$exe=Join-Path $unc 'Открыть_производственный_цикл.exe'
$certificate=$null;$shareCreated=$false
$evidence=@{serverBase='b7dbce3';shareReadOnly=$false;uncPath=$unc;events=@();currentUserTrust=$false;serverAndFrontendPreserved=$true;networkOfOwnerVerified=$false}
$out=Join-Path $env:GITHUB_WORKSPACE 'dist/shared-qa'
New-Item -ItemType Directory -Path $out -Force | Out-Null
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class SharedDialog {
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls,string title);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr parent,int id);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr hwnd,uint message,IntPtr wparam,IntPtr lparam);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd,StringBuilder text,int length);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int length);
 [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hwnd);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 delegate bool EnumProc(IntPtr hwnd,IntPtr state);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent,EnumProc callback,IntPtr state);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback,IntPtr state);
 public static string Text(IntPtr parent){var result=new StringBuilder();EnumChildWindows(parent,(h,s)=>{var t=new StringBuilder(4096);GetWindowText(h,t,t.Capacity);result.AppendLine(GetDlgCtrlID(h)+": "+t.ToString());return true;},IntPtr.Zero);return result.ToString();}
 public static IntPtr NamedButton(IntPtr parent,string name){IntPtr result=IntPtr.Zero;EnumChildWindows(parent,(h,s)=>{var t=new StringBuilder(256);var c=new StringBuilder(256);GetWindowText(h,t,t.Capacity);GetClassName(h,c,c.Capacity);if(c.ToString().IndexOf("BUTTON",StringComparison.OrdinalIgnoreCase)>=0&&t.ToString().Replace("&","")==name)result=h;return true;},IntPtr.Zero);return result;}
 public static string Windows(){var result=new StringBuilder();EnumWindows((h,s)=>{var t=new StringBuilder(512);var c=new StringBuilder(256);GetWindowText(h,t,t.Capacity);GetClassName(h,c,c.Capacity);uint pid;GetWindowThreadProcessId(h,out pid);if(t.Length>0)result.AppendLine(pid+" | "+c+" | "+t+" | "+Text(h));return true;},IntPtr.Zero);return result.ToString();}
}
'@
function Invoke-WindowsPowerShell {
 param([string]$File,[string]$Arguments)
 $info=New-Object Diagnostics.ProcessStartInfo
 $info.FileName=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
 $info.Arguments='-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+$File+'" '+$Arguments
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.EnvironmentVariables['PSModulePath']=@((Join-Path $env:ProgramFiles 'WindowsPowerShell/Modules'),(Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/Modules')) -join ';'
 $process=[Diagnostics.Process]::Start($info)
 try{
  $output=$process.StandardOutput.ReadToEndAsync();$errors=$process.StandardError.ReadToEndAsync()
  if(-not $process.WaitForExit(60000)){Stop-Process -Id $process.Id;throw 'Windows PowerShell setup exceeded 60 seconds'}
  $output.Result | Write-Host;$errors.Result | Write-Host
  if($process.ExitCode -ne 0){throw ('Windows PowerShell operation failed: '+$process.ExitCode)}
 }finally{$process.Dispose()}
}
function Invoke-SharedButton {
 param([string]$Dialog,[int]$Button,[int]$ExpectedExit=0,[string]$ExpectedText,[string]$NativeThumbprint)
 $info=New-Object Diagnostics.ProcessStartInfo
 $info.FileName=$exe;$info.UseShellExecute=$false
 $info.EnvironmentVariables['PSModulePath']=@((Join-Path $env:ProgramFiles 'WindowsPowerShell/Modules'),(Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/Modules')) -join ';'
 $process=[Diagnostics.Process]::Start($info)
 $deadline=[DateTime]::UtcNow.AddSeconds(35);$seen=$false
 try{
  while(-not $process.WaitForExit(100)){
   $confirm=[SharedDialog]::FindWindow('#32770','Первое подключение к производственному циклу')
   $errorDialog=[SharedDialog]::FindWindow('#32770','Не удалось открыть производственный цикл')
   $nativeDialog=[SharedDialog]::FindWindow('#32770','Security Warning')
   if($nativeDialog -ne [IntPtr]::Zero){
    $nativeText=[SharedDialog]::Text($nativeDialog)
    if(-not $NativeThumbprint -or -not $seen -or -not $nativeText.Replace(' ','').Contains($NativeThumbprint)){throw ('Unexpected Windows certificate dialog: '+$nativeText)}
    Write-Host 'ACCEPTANCE: confirming the Windows certificate prompt after matching the generated SHA1'
    $nativeButton=[SharedDialog]::GetDlgItem($nativeDialog,6)
    $null=[SharedDialog]::SetForegroundWindow($nativeDialog)
    $null=[SharedDialog]::SendMessage($nativeDialog,0x0111,[IntPtr]6,$nativeButton)
   }
   if($confirm -ne [IntPtr]::Zero -and $Dialog -ne 'confirm'){throw 'Unexpected trust confirmation on repeat launch'}
   if($errorDialog -ne [IntPtr]::Zero -and $Dialog -ne 'error'){throw ('Unexpected launch error: '+[SharedDialog]::Text($errorDialog))}
   $handle=if($Dialog -eq 'confirm'){$confirm}elseif($Dialog -eq 'error'){$errorDialog}else{[IntPtr]::Zero}
   if($handle -ne [IntPtr]::Zero){
    $text=[SharedDialog]::Text($handle)
    $control=[SharedDialog]::GetDlgItem($handle,$Button)
    if($control -eq [IntPtr]::Zero -and $Button -eq 1){$control=[SharedDialog]::NamedButton($handle,'OK')}
    # The native dialog can exist before all of its child controls are initialized.
    if($control -ne [IntPtr]::Zero -and [SharedDialog]::IsWindowVisible($handle) -and [SharedDialog]::IsWindowEnabled($control) -and (-not $ExpectedText -or $text.Contains($ExpectedText))){
     if(-not $seen){Write-Host ("ACCEPTANCE: dialog observed, selecting button "+$Button)}
     $null=[SharedDialog]::SetForegroundWindow($handle)
     $null=[SharedDialog]::SendMessage($control,0x00F5,[IntPtr]::Zero,[IntPtr]::Zero)
     $seen=$true
    }
   }
   if([DateTime]::UtcNow -gt $deadline){
    Write-Host ([SharedDialog]::Windows())
    Get-CimInstance Win32_Process | Where-Object {$_.Name -in @('powershell.exe','Открыть_производственный_цикл.exe')} | Select-Object Name,ProcessId,ParentProcessId,SessionId,CommandLine | Format-List | Out-String | Write-Host
    throw 'Shared folder button did not finish in 35 seconds'
   }
  }
  if($Dialog -and -not $seen){throw 'Expected dialog did not appear'}
  if($process.ExitCode -ne $ExpectedExit){throw "Unexpected launcher exit $($process.ExitCode), expected $ExpectedExit"}
 }finally{if(-not $process.HasExited){& taskkill.exe /PID $process.Id /T /F | Out-Null};$process.Dispose()}
}
function Get-SharedReadHash {
 param([Parameter(Mandatory=$true)][string]$Path)
 $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
 $sha=[Security.Cryptography.SHA256]::Create()
 try{return ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','')}finally{$sha.Dispose();$stream.Dispose()}
}
try{
 $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
 $address=$env:COMPUTERNAME.ToLowerInvariant()
 Write-Host 'ACCEPTANCE: configure the unchanged base server as a Windows service'
 Invoke-WindowsPowerShell -File (Join-Path $root 'scripts/server/Configure-Easy.ps1') -Arguments "-Address $address -Port $port -RemoteAddresses LocalSubnet -Accept -NoBrowser"
 . (Join-Path $root 'scripts/server/Common.ps1')
 $certificate=New-Object Security.Cryptography.X509Certificates.X509Certificate2((Join-Path $root 'server.cer'))
 if(Test-Path -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint)){throw 'The generated test certificate is unexpectedly trusted before the test'}
 $metadataPath=Join-Path $destination 'Служебные_файлы/client.json'
 Invoke-WindowsPowerShell -File (Join-Path $root 'scripts/server/Export-SharedFolder.ps1') -Arguments ('-Destination "'+$destination+'" -NoOpen')
 $metadata=Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
 $expected=@('Открыть_производственный_цикл.exe','ПРОЧИТАЙТЕ.html','Служебные_файлы/Browser.ps1','Служебные_файлы/SharedFolder.ps1','Служебные_файлы/Open-SharedFolder.ps1','Служебные_файлы/client.json','Служебные_файлы/server.cer') | Sort-Object
 $actual=Get-ChildItem -LiteralPath $destination -File -Recurse | ForEach-Object {$_.FullName.Substring($destination.Length+1).Replace('\','/')} | Sort-Object
 if(@(Compare-Object $expected $actual).Count){throw 'The shared launch folder contains unexpected files or missing client files'}
 Start-Service LanmanServer
 New-SmbShare -Name $shareName -Path $public -ReadAccess ([Security.Principal.WindowsIdentity]::GetCurrent().Name) | Out-Null
 $shareCreated=$true
 if(-not(Test-Path -LiteralPath $exe)){throw 'The generated client cannot be read through SMB'}
 $blocked=$false
 try{[IO.File]::WriteAllText((Join-Path $unc 'must-not-write.tmp'),'no')}catch [UnauthorizedAccessException]{$blocked=$true}
 if(-not $blocked){throw 'The shared launch folder is writable by the SMB client'}
 $evidence.shareReadOnly=$true
 Write-Host 'ACCEPTANCE: the exact GUI EXE runs from a read-only Cyrillic UNC path'
 Invoke-SharedButton -Dialog confirm -Button 7 -ExpectedExit 2 -ExpectedText $metadata.certificateSha256
 if(Test-Path -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint)){throw 'Rejecting the prompt must not install certificate trust'}
 $evidence.events+='first-click-refused-without-trust'
 Invoke-SharedButton -Dialog confirm -Button 6 -ExpectedText $metadata.certificateSha256 -NativeThumbprint $certificate.Thumbprint
 if(-not(Test-Path -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint))){throw 'Accepting the prompt did not install CurrentUser trust'}
 $evidence.currentUserTrust=$true;$evidence.events+='first-click-accepted'
 $deadline=[DateTime]::UtcNow.AddSeconds(15)
 do{$edgeWindow=Get-Process msedge -ErrorAction SilentlyContinue | Where-Object {$_.MainWindowHandle -ne 0};if($edgeWindow){break};Start-Sleep -Milliseconds 150}while([DateTime]::UtcNow -lt $deadline)
 if(-not $edgeWindow){throw 'The real installed Edge did not open a GUI window'}
 Invoke-SharedButton
 $evidence.events+='repeat-click-without-prompt';$evidence.events+='native-edge-window'
 $env:TRUSTED_SERVER_URL=$metadata.url
 $env:SERVER_QA_DIR=$out
 node (Join-Path $PSScriptRoot '../tests/server/shared-browser.cjs')
 if($LASTEXITCODE -ne 0){throw 'Normal Edge certificate verification or offline asset loading failed'}
 Write-Host 'ACCEPTANCE: reject inconsistent certificates before opening the browser'
 $original=[IO.File]::ReadAllText($metadataPath)
 $metadata.certificateSha256='0'*64
 [IO.File]::WriteAllText($metadataPath,($metadata|ConvertTo-Json),(New-Object Text.UTF8Encoding($false)))
 Invoke-SharedButton -Dialog error -Button 1 -ExpectedExit 1 -ExpectedText 'не соответствует'
 [IO.File]::WriteAllText($metadataPath,$original,(New-Object Text.UTF8Encoding($false)))
 $evidence.events+='certificate-mismatch-rejected'
 Write-Host 'ACCEPTANCE: stop the service and show a bounded, useful connection error'
 Stop-Service ProductionCycleServer;(Get-Service ProductionCycleServer).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(30))
 Invoke-SharedButton -Dialog error -Button 1 -ExpectedExit 1 -ExpectedText $env:TRUSTED_SERVER_URL
 $evidence.events+='unavailable-service-error'
 Start-Service ProductionCycleServer
 $deadline=[DateTime]::UtcNow.AddSeconds(20)
 do{if((Invoke-ProductionExe -Arguments @('--check') -Root $root -Quiet) -eq 0){break};Start-Sleep -Milliseconds 150}while([DateTime]::UtcNow -lt $deadline)
 Write-Host 'ACCEPTANCE: refresh the same public folder without modifying database or private keys'
 $dataHash=Get-SharedReadHash -Path (Join-Path $root 'server-data/production-cycle.db')
 $keyHash=(Get-FileHash -LiteralPath (Join-Path $root 'tls/server-key.pem') -Algorithm SHA256).Hash
 Invoke-WindowsPowerShell -File (Join-Path $root 'scripts/server/Export-SharedFolder.ps1') -Arguments ('-Destination "'+$destination+'" -NoOpen')
 if((Get-SharedReadHash -Path (Join-Path $root 'server-data/production-cycle.db')) -cne $dataHash){throw 'Export modified the database'}
 if((Get-FileHash -LiteralPath (Join-Path $root 'tls/server-key.pem') -Algorithm SHA256).Hash -cne $keyHash){throw 'Export modified the TLS private key'}
 Invoke-SharedButton
 $evidence.events+='folder-refreshed-without-data-change'
 Write-Host 'PASS: shared folder GUI launcher, read-only SMB, CurrentUser trust, native Edge, repeat click and error handling'
}catch{
 $evidence.error=$_.Exception.Message
 throw
}finally{
 $evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $out 'shared-folder.json') -Encoding UTF8
 if($shareCreated){Remove-SmbShare -Name $shareName -Force}
 if(Get-Service ProductionCycleServer -ErrorAction SilentlyContinue){& (Join-Path $root 'scripts/server/Remove-Service.ps1')}
 if($certificate -and (Test-Path -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint))){
  # Cleanup must never wait on a second unattended Windows trust dialog.
  $cleanupInfo=New-Object Diagnostics.ProcessStartInfo
  $cleanupInfo.FileName=Join-Path $env:SystemRoot 'System32/certutil.exe'
  $cleanupInfo.Arguments='-f -silent -user -delstore Root '+$certificate.Thumbprint
  $cleanupInfo.UseShellExecute=$false;$cleanupInfo.CreateNoWindow=$true
  $cleanupInfo.RedirectStandardOutput=$true;$cleanupInfo.RedirectStandardError=$true
  $cleanup=[Diagnostics.Process]::Start($cleanupInfo)
  try{
   $cleanupOut=$cleanup.StandardOutput.ReadToEndAsync();$cleanupError=$cleanup.StandardError.ReadToEndAsync()
   if(-not $cleanup.WaitForExit(10000)){Stop-Process -Id $cleanup.Id;Write-Warning 'Temporary root cleanup timed out on the disposable CI runner'}
   elseif($cleanup.ExitCode -ne 0){Write-Warning ('Temporary root cleanup: '+$cleanupError.Result)}
  }finally{$cleanup.Dispose()}
 }
 if(Get-NetFirewallRule -Name ProductionCycleServer-HTTPS -ErrorAction SilentlyContinue){Remove-NetFirewallRule -Name ProductionCycleServer-HTTPS}
}
