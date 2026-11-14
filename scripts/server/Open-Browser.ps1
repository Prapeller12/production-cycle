param([switch]$Local)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Browser.ps1')
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$config=Get-Content (Join-Path $root 'config/server.json') -Raw | ConvertFrom-Json
$url=$config.publicUrl
if($Local){$uri=[Uri]$url;$url=$uri.Scheme+'://localhost:'+$uri.Port}
if($url -notmatch '^https?://[A-Za-z0-9.-]+(?::\d+)?/?$'){throw 'Некорректный адрес в настройках'}
Open-ProductionBrowser -Url $url
