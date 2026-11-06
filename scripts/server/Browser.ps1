function Get-ProductionBrowser {
 foreach($base in @(${env:ProgramFiles(x86)},$env:ProgramFiles,$env:LOCALAPPDATA)){
  if($base){$path=Join-Path $base 'Microsoft/Edge/Application/msedge.exe';if(Test-Path -LiteralPath $path){return @{Path=$path;Edge=$true}}}
 }
 if($env:ProgramFiles){$path=Join-Path $env:ProgramFiles 'Internet Explorer/iexplore.exe';if(Test-Path -LiteralPath $path){return @{Path=$path;Edge=$false}}}
 throw 'Не найден установленный Edge или Internet Explorer 11. Для работы нужен один из этих браузеров.'
}
function Open-ProductionBrowser {
 param([Parameter(Mandatory=$true)][string]$Url)
 $uri=[Uri]$Url
 if($uri.Scheme -notin @('http','https') -or $uri.UserInfo -or $uri.Query -or $uri.Fragment -or $uri.AbsolutePath -ne '/'){throw 'Некорректный адрес программы'}
 $browser=Get-ProductionBrowser
 if($browser.Edge){Start-Process -FilePath $browser.Path -ArgumentList @('--new-window',$Url)}else{Start-Process -FilePath $browser.Path -ArgumentList $Url}
}
