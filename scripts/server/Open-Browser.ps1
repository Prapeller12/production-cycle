param([switch]$Local)
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$config=Get-Content (Join-Path $root 'config/server.json') -Raw | ConvertFrom-Json
$url=$config.publicUrl
if($Local){$uri=[Uri]$url;$url=$uri.Scheme+'://localhost:'+$uri.Port}
if($url -notmatch '^https?://[A-Za-z0-9.-]+(?::\d+)?/?$'){throw 'Некорректный адрес в настройках'}
$edge=@("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe","$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe","$env:LOCALAPPDATA\Microsoft\Edge\Application\msedge.exe") | Where-Object {Test-Path -LiteralPath $_} | Select-Object -First 1
if($edge){Start-Process -FilePath $edge -ArgumentList $url;exit}
$ie=Join-Path $env:ProgramFiles 'Internet Explorer/iexplore.exe'
if(Test-Path -LiteralPath $ie){Start-Process -FilePath $ie -ArgumentList $url;exit}
Start-Process $url
