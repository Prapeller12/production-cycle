param([Parameter(Mandatory=$true)][string]$Zip)
$ErrorActionPreference='Stop'
$testRoot=Join-Path $env:RUNNER_TEMP 'Сервер браузер служба'
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $testRoot -Force
$root=Join-Path $testRoot 'ProductionCycleServer'
$listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
$file=Join-Path $root 'config/server.json'
$address=Get-NetIPAddress -AddressFamily IPv4 | Where-Object {$_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*'} | Select-Object -First 1 -ExpandProperty IPAddress
if(-not $address){throw 'No network IPv4 available to exercise server setup'}
$certificate=$null
try{
 & (Join-Path $root 'scripts/server/Configure-Easy.ps1') -Address $address -Port $port -RemoteAddresses '127.0.0.1' -Accept -NoBrowser
 $config=Get-Content $file -Raw | ConvertFrom-Json
 $deadline=[DateTime]::UtcNow.AddSeconds(30);$ready=$false
 do{try{$r=Invoke-WebRequest "$($config.publicUrl)/health" -SkipCertificateCheck;if($r.StatusCode -eq 200){$ready=$true;break}}catch{};Start-Sleep -Milliseconds 200}while([DateTime]::UtcNow -lt $deadline)
 if(-not $ready){throw 'Service has no ready HTTP endpoint'}
 $service=Get-CimInstance Win32_Service -Filter "Name='ProductionCycleServer'"
 if($service.StartName -notmatch 'LocalService|Local Service'){throw "Unexpected service identity: $($service.StartName)"}
 if(-not(Test-Path (Join-Path $root 'server-data/production-cycle.db'))){throw 'Service did not create its local database'}
 . (Join-Path $root 'scripts/server/Common.ps1')
 if((Invoke-ProductionExe -Arguments @('--check') -Root $root) -ne 0){throw 'Native readiness check failed for the service'}
 if((Invoke-ProductionExe -Arguments @('--stop') -Root $root) -eq 0){throw 'Manual shutdown must not stop a Windows service'}
 & (Join-Path $root 'scripts/server/Export-Client.ps1')
 $client=Join-Path $root 'Подключение_сотрудника'
 $metadata=Get-Content (Join-Path $client 'client.json') -Raw | ConvertFrom-Json
 if($metadata.url -ne $config.publicUrl){throw 'Client URL mismatch'}
 if($metadata.certificateSha256 -ne (Get-FileHash (Join-Path $client 'server.cer') -Algorithm SHA256).Hash.ToLowerInvariant()){throw 'Client certificate hash mismatch'}
 $allowed=@('server.cer','client.json','Browser.ps1','Connect-Client.ps1','Open-Client.ps1','Подключить.cmd','Открыть_программу.cmd','ПРОЧИТАЙТЕ.txt')
 foreach($entry in Get-ChildItem -LiteralPath $client -Recurse){if($entry.Name -notin $allowed){throw "Unexpected client file: $($entry.Name)"}}
 $certificate=Import-Certificate -FilePath (Join-Path $client 'server.cer') -CertStoreLocation 'Cert:\CurrentUser\Root'
 $env:TRUSTED_SERVER_URL="https://localhost:$port"
 node (Join-Path $PSScriptRoot '../tests/server/trusted-browser.cjs')
 if($LASTEXITCODE -ne 0){throw 'Trusted client Edge acceptance failed'}
 Stop-Service ProductionCycleServer;(Get-Service ProductionCycleServer).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(30))
 Start-Service ProductionCycleServer
 & (Join-Path $root 'scripts/server/Configure-Easy.ps1') -Address $address -Port $port -RemoteAddresses '127.0.0.1' -Accept -NoBrowser
 if(-not(Test-Path (Join-Path $root 'server-data/production-cycle.db'))){throw 'Reconfiguration lost the database'}
 Write-Host 'PASS: packaged EXE runs as LocalService, creates local data, serves HTTPS and restarts'
}finally{
 if(Get-Service ProductionCycleServer -ErrorAction SilentlyContinue){& (Join-Path $root 'scripts/server/Remove-Service.ps1')}
 if($certificate){Remove-Item -LiteralPath ('Cert:\CurrentUser\Root\'+$certificate.Thumbprint)}
 if(Get-NetFirewallRule -Name ProductionCycleServer-HTTPS -ErrorAction SilentlyContinue){Remove-NetFirewallRule -Name ProductionCycleServer-HTTPS}
}
