param([Parameter(Mandatory=$true)][string]$Zip)
$ErrorActionPreference='Stop'
$testRoot=Join-Path $env:RUNNER_TEMP 'Сервер браузер служба'
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $testRoot -Force
$root=Join-Path $testRoot 'ProductionCycleServer'
$listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
$file=Join-Path $root 'config/server.json';$config=Get-Content $file -Raw | ConvertFrom-Json
$config.listen="127.0.0.1:$port";$config.publicUrl="https://localhost:$port"
[IO.File]::WriteAllText($file,($config|ConvertTo-Json),(New-Object Text.UTF8Encoding($false)))
try{
 & (Join-Path $root 'scripts/server/Install-Service.ps1')
 $deadline=[DateTime]::UtcNow.AddSeconds(30);$ready=$false
 do{try{$r=Invoke-WebRequest "$($config.publicUrl)/health" -SkipCertificateCheck;if($r.StatusCode -eq 200){$ready=$true;break}}catch{};Start-Sleep -Milliseconds 200}while([DateTime]::UtcNow -lt $deadline)
 if(-not $ready){throw 'Service has no ready HTTP endpoint'}
 $service=Get-CimInstance Win32_Service -Filter "Name='ProductionCycleServer'"
 if($service.StartName -notmatch 'LocalService|Local Service'){throw "Unexpected service identity: $($service.StartName)"}
 if(-not(Test-Path (Join-Path $root 'server-data/production-cycle.db'))){throw 'Service did not create its local database'}
 Stop-Service ProductionCycleServer;(Get-Service ProductionCycleServer).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(30))
 Start-Service ProductionCycleServer
 Write-Host 'PASS: packaged EXE runs as LocalService, creates local data, serves HTTPS and restarts'
}finally{if(Get-Service ProductionCycleServer -ErrorAction SilentlyContinue){& (Join-Path $root 'scripts/server/Remove-Service.ps1')}}
