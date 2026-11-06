$script:ProductionRoot=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
function Get-ProductionExe {
 param([string]$Root=$script:ProductionRoot)
 foreach($name in @('Производственный_цикл.exe','production-cycle-server.exe')){
  $path=Join-Path $Root $name;if(Test-Path -LiteralPath $path){return $path}
 }
 throw 'Не найден файл запуска. Распакуйте ZIP целиком.'
}
function Invoke-ProductionExe {
 param([string[]]$Arguments,[string]$Root=$script:ProductionRoot)
 $exe=Get-ProductionExe -Root $Root
 $process=Start-Process -FilePath $exe -ArgumentList $Arguments -Wait -PassThru -NoNewWindow
 return $process.ExitCode
}
function Get-ProductionCertificateHash {
 param([string]$Path)
 return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
