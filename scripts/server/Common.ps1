$script:ProductionRoot=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
function Get-ProductionExe {
 param([string]$Root=$script:ProductionRoot)
 foreach($name in @('Производственный_цикл.exe','production-cycle-server.exe')){
  $path=Join-Path $Root $name;if(Test-Path -LiteralPath $path){return $path}
 }
 throw 'Не найден файл запуска. Распакуйте ZIP целиком.'
}
function Invoke-ProductionExe {
 param([string[]]$Arguments,[string]$Root=$script:ProductionRoot,[switch]$Quiet)
 $exe=Get-ProductionExe -Root $Root
 # Windows PowerShell 5.1 Start-Process may return an uncached/null ExitCode.
 # Start through ProcessStartInfo so the process handle and its exit status stay owned here.
 $info=New-Object Diagnostics.ProcessStartInfo
 $info.FileName=$exe;$info.Arguments=$Arguments -join ' ';$info.UseShellExecute=$false;$info.CreateNoWindow=$true
 $info.RedirectStandardOutput=[bool]$Quiet;$info.RedirectStandardError=[bool]$Quiet
 $process=[Diagnostics.Process]::Start($info)
 try{
  if($Quiet){$null=$process.StandardOutput.ReadToEnd();$null=$process.StandardError.ReadToEnd()}
  $process.WaitForExit()
  return $process.ExitCode
 }finally{$process.Dispose()}
}
function Get-ProductionCertificateHash {
 param([string]$Path)
 return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
