param([Parameter(Mandatory=$true)][string]$Root,[switch]$RealWindows)
$ErrorActionPreference='Stop'
$rootPath=(Resolve-Path $Root).Path
$argument=if($RealWindows){'--verify-windows'}else{'--self-test'}
$report=Join-Path $rootPath 'temp/frame-test.json'
if(Test-Path $report){Remove-Item $report}
$process=Start-Process -FilePath (Join-Path $rootPath 'ProductionFrame.exe') -ArgumentList $argument -PassThru
if(-not $process.WaitForExit(160000)) {
    # CI runs only against newly extracted, disposable fixtures. Never kill a user process here.
    throw 'Frame did not exit cleanly; keep evidence and fail this test.'
}
if($process.ExitCode -ne 0){throw "Frame exit code: $($process.ExitCode)"}
if(-not(Test-Path $report)){throw 'No frame test evidence'}
$result=Get-Content $report -Raw | ConvertFrom-Json
if(-not $result.ok -or $result.step -ne 4){throw 'Frame integration checks failed'}
Get-Content $report
