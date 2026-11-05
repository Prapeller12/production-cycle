# CI diagnostics only. No trace binary or capture is included in the app ZIP.
$ErrorActionPreference='Stop'
function Start-StandardUserTrace {
  if($env:PRODUCTION_CYCLE_CI_TRACE -ne '1'){return $null}
  $folder=Join-Path $env:RUNNER_TEMP 'pc-startup-trace';New-Item -ItemType Directory $folder -Force | Out-Null
  $zip=Join-Path $folder 'ProcessMonitor.zip'
  Invoke-WebRequest 'https://download.sysinternals.com/files/ProcessMonitor.zip' -OutFile $zip
  if((Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne '477754bf299c9e5b311c5666f7a4a48dfdb21265eaa8f0cca70ef9d9552e3a10'){throw 'Unexpected Microsoft Process Monitor ZIP digest'}
  Expand-Archive $zip (Join-Path $folder 'tool')
  $exe=Join-Path $folder 'tool/Procmon64.exe'
  $signature=Get-AuthenticodeSignature $exe
  if($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation'){throw 'Microsoft Process Monitor signature is invalid'}
  $pml=Join-Path $folder 'startup.pml';$csv=Join-Path $folder 'startup.csv'
  $process=Start-Process $exe -ArgumentList @('/AcceptEula','/Quiet','/Minimized','/NoFilter','/BackingFile',('"'+$pml+'"')) -PassThru
  Start-Sleep -Seconds 2
  $stop=Start-ThreadJob -ScriptBlock {param($tool) Start-Sleep -Seconds 90;& $tool /Terminate /Quiet} -ArgumentList $exe
  return @{exe=$exe;pml=$pml;csv=$csv;process=$process;stop=$stop}
}
function Stop-StandardUserTrace($trace,[string]$destination){
  if($null -eq $trace){return}
  & $trace.exe /Terminate /Quiet
  if(-not $trace.process.WaitForExit(30000)){throw 'Process Monitor did not stop'}
  $export=Start-Process $trace.exe -ArgumentList @('/AcceptEula','/Quiet','/OpenLog',('"'+$trace.pml+'"'),'/SaveAs',('"'+$trace.csv+'"')) -PassThru
  if(-not $export.WaitForExit(60000)){Stop-Process -Id $export.Id -Force;throw 'Process Monitor CSV export timed out'}
  Import-Csv $trace.csv | Where-Object {$_.'Process Name' -in @('production-cycle.exe','msedgewebview2.exe')} | Export-Csv $destination -NoTypeInformation -Encoding utf8
  Receive-Job $trace.stop -Wait -AutoRemoveJob | Out-Null
}
