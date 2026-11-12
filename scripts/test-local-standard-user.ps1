param([Parameter(Mandatory=$true)][string]$Zip)
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true'){throw 'Standard account fixture is CI-only'}
$root=Join-Path $env:RUNNER_TEMP 'Local standard user'
Expand-Archive $Zip $root
$name='PcLocalQA'+(Get-Random -Maximum 9999)
$secret=[guid]::NewGuid().ToString()+'aA1!'
$created=$false;$configured=$false
try{
 New-LocalUser -Name $name -Password (ConvertTo-SecureString $secret -AsPlainText -Force) -AccountNeverExpires -PasswordNeverExpires | Out-Null
 $created=$true
 Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $name
 $acl=Get-Acl $root
 $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new("$env:COMPUTERNAME\$name",'Modify','ContainerInherit, ObjectInherit','None','Allow'))
 Set-Acl $root $acl
 Start-Service seclogon
 Add-Type -Path (Join-Path $PSScriptRoot 'launch-limited-token.cs')
 Copy-Item (Join-Path $PSScriptRoot 'launch-limited-token.cs') (Join-Path $root 'network-standard-user-probe.cs')
 $env:PRODUCTION_CYCLE_SMOKE_REPORT=Join-Path $root 'native-standard-user.json'
 [LimitedLaunch]::Configure($name,$secret);$configured=$true
 $process=[LimitedLaunch]::Start((Join-Path $root 'ProductionCycle/production-cycle.exe'),'--smoke-test',$root)
 $end=[DateTime]::UtcNow.AddSeconds(200)
 while(-not $process.HasExited -and [DateTime]::UtcNow -lt $end){Start-Sleep -Milliseconds 250}
 if(-not $process.HasExited){Stop-Process -Id $process.Id -Force;throw 'Standard-account EXE timed out'}
 if(-not(Test-Path $env:PRODUCTION_CYCLE_SMOKE_REPORT)){throw 'No standard-account native report'}
 $r=Get-Content $env:PRODUCTION_CYCLE_SMOKE_REPORT -Raw | ConvertFrom-Json
 if(-not $r.ok -or $process.ExitCode -ne 0 -or -not $r.windowVisible){throw "Standard-account EXE failed: $($r.error)"}
 New-Item -ItemType Directory -Path ./dist/local-window-qa -Force | Out-Null
 Copy-Item $env:PRODUCTION_CYCLE_SMOKE_REPORT ./dist/local-window-qa/native-standard-user.json
 Copy-Item "$root/network-standard-user-identity.txt" ./dist/local-window-qa/native-standard-user-identity.txt
 Write-Host 'PASS: real standard Windows account, profile loaded, visible local EXE, signed SQLite and Excel native IPC'
}finally{
 if($configured){[LimitedLaunch]::Restore()}
 Remove-Item Env:PRODUCTION_CYCLE_SMOKE_REPORT -ErrorAction SilentlyContinue
 if($created){Remove-LocalUser -Name $name -ErrorAction SilentlyContinue}
}
