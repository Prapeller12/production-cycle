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
 delegate bool EnumProc(IntPtr hwnd,IntPtr state);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent,EnumProc callback,IntPtr state);
 public static string Text(IntPtr parent){var result=new StringBuilder();EnumChildWindows(parent,(h,s)=>{var t=new StringBuilder(4096);GetWindowText(h,t,t.Capacity);result.AppendLine(t.ToString());return true;},IntPtr.Zero);return result.ToString();}
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
 param([string]$Dialog,[int]$Button,[int]$ExpectedExit=0,[string]$ExpectedText)
 $info=New-Object Diagnostics.ProcessStartInfo
 $info.FileName=$exe;$info.UseShellExecute=$false
 $info.EnvironmentVariables['PSModulePath']=@((Join-Path $env:ProgramFiles 'WindowsPowerShell/Modules'),(Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/Modules')) -join ';'
 $process=[Diagnostics.Process]::Start($info)
 $deadline=[DateTime]::UtcNow.AddSeconds(35);$seen=$false
 try{
  while(-not $process.WaitForExit(100)){
   $confirm=[SharedDialog]::FindWindow('#32770','Первое подключение к производственному циклу')
   $errorDialog=[SharedDialog]::FindWindow('#32770','Не удалось открыть производственный цикл')
   if($confirm -ne [IntPtr]::Zero -and $Dialog -ne 'confirm'){throw 'Unexpected trust confirmation on repeat launch'}
   if($errorDialog -ne [IntPtr]::Zero -and $Dialog -ne 'error'){throw ('Unexpected launch error: '+[SharedDialog]::Text($errorDialog))}
   $handle=if($Dialog -eq 'confirm'){$confirm}elseif($Dialog -eq 'error'){$errorDialog}else{[IntPtr]::Zero}
   if(-not $seen -and $handle -ne [IntPtr]::Zero){
    $text=[SharedDialog]::Text($handle)
    if($ExpectedText -and -not $text.Contains($ExpectedText)){throw ('Dialog omitted the expected information: '+$text)}
    $control=[SharedDialog]::GetDlgItem($handle,$Button)
    if($control -eq [IntPtr]::Zero){throw 'Expected message box button missing'}
    $null=[SharedDialog]::SendMessage($control,0x00F5,[IntPtr]::Zero,[IntPtr]::Zero)
    $seen=$true
   }
   if([DateTime]::UtcNow -gt $deadline){throw 'Shared folder button did not finish in 35 seconds'}
  }
  if($Dialog -and -not $seen){throw 'Expected dialog did not appear'}
  if($process.ExitCode -ne $ExpectedExit){throw "Unexpected launcher exit $($process.ExitCode), expected $ExpectedExit"}
 }finally{if(-not $process.HasExited){& taskkill.exe /PID $process.Id /T /F | Out-Null};$process.Dispose()}
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
 Invoke-SharedButton -Dialog confirm -Button 6 -ExpectedText $metadata.certificateSha256
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
 $dataHash=(Get-FileHash -LiteralPath (Join-Path $root 'server-data/production-cycle.db') -Algorithm SHA256).Hash
 $keyHash=(Get-FileHash -LiteralPath (Join-Path $root 'tls/server-key.pem') -Algorithm SHA256).Hash
 Invoke-WindowsPowerShell -File (Join-Path $root 'scripts/server/Export-SharedFolder.ps1') -Arguments ('-Destination "'+$destination+'" -NoOpen')
 if((Get-FileHash -LiteralPath (Join-Path $root 'server-data/production-cycle.db') -Algorithm SHA256).Hash -cne $dataHash){throw 'Export modified the database'}
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
 if($certificate -and (Test-Path -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint))){Remove-Item -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint)}
 if(Get-NetFirewallRule -Name ProductionCycleServer-HTTPS -ErrorAction SilentlyContinue){Remove-NetFirewallRule -Name ProductionCycleServer-HTTPS}
}
