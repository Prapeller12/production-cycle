param([Parameter(Mandatory=$true)][string]$Root)
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true'){throw 'Diagnostic fault fixtures are CI-only'}
$Root=(Resolve-Path -LiteralPath $Root).Path
$out=Join-Path (Get-Location) 'dist/startup-qa';New-Item -ItemType Directory -Path $out -Force | Out-Null
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class StartupProbe {
 public delegate bool Callback(IntPtr window,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(Callback cb,IntPtr arg);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr w,StringBuilder text,int count);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr w,out uint pid);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr w);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr w,int id);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr w,uint msg,IntPtr wp,IntPtr lp);
 public static IntPtr Find(int pid) {IntPtr result=IntPtr.Zero;EnumWindows((w,a)=>{uint owner;GetWindowThreadProcessId(w,out owner);var text=new StringBuilder(256);GetWindowTextW(w,text,256);if(owner==pid&&IsWindowVisible(w)&&text.ToString()=="Производственный цикл — ошибка запуска"){result=w;return false;}return true;},IntPtr.Zero);return result;}
 public static string Read(IntPtr w){var text=new StringBuilder(32768);GetWindowTextW(GetDlgItem(w,1001),text,text.Capacity);return text.ToString();}
}
'@
$oldReport=$env:PRODUCTION_CYCLE_SMOKE_REPORT;$oldTimeout=$env:PRODUCTION_CYCLE_TEST_STARTUP_TIMEOUT_MS
function Expect-VisibleFailure([string]$name,[string]$pattern){
 $report=Join-Path $out ($name+'.json');$env:PRODUCTION_CYCLE_SMOKE_REPORT=$report
 $process=Start-Process -FilePath "$Root/production-cycle.exe" -ArgumentList '--smoke-test' -WorkingDirectory "$env:SystemRoot/System32" -PassThru
 $deadline=[DateTime]::UtcNow.AddSeconds(40);$dialog=[IntPtr]::Zero
 while([DateTime]::UtcNow -lt $deadline -and -not $process.HasExited){$dialog=[StartupProbe]::Find($process.Id);if($dialog -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}
 if($dialog -eq [IntPtr]::Zero){Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue;throw "No visible diagnostic: $name"}
 $text=[StartupProbe]::Read($dialog)
 if($text -notmatch $pattern -or $text -notmatch 'Этап:' -or $text -notmatch 'EXE:' -or $text -notmatch 'Рабочая папка ярлыка:'){throw "Incomplete visible diagnostic $name : $text"}
 $text | Set-Content (Join-Path $out ($name+'-visible.txt')) -Encoding utf8
 [StartupProbe]::PostMessageW($dialog,0x0111,[IntPtr]1,[IntPtr]::Zero)|Out-Null
 if(-not $process.WaitForExit(15000)){Stop-Process -Id $process.Id -Force;throw "Diagnostic cannot close: $name"}
 $value=Get-Content $report -Raw|ConvertFrom-Json
 if($value.ok -or $process.ExitCode -eq 0){throw "Failure reported success: $name"}
 Write-Host "PASS visible native diagnostic: $name"
}
$app=Join-Path $Root 'frontend/ui/app.js';$original=[IO.File]::ReadAllBytes($app)
try{
 Move-Item $app ($app+'.saved')
 try{Expect-VisibleFailure 'missing-interface-file' '(?s)Недоступен обязательный файл интерфейса.*app.js.*Код ОС:'}finally{Move-Item ($app+'.saved') $app}
 [IO.File]::WriteAllText($app,"throw new Error('STARTUP_UI_PROBE');",[Text.UTF8Encoding]::new($false))
 Expect-VisibleFailure 'frontend-script-error' 'STARTUP_UI_PROBE'
 [IO.File]::WriteAllText($app,'window.nativeStartupProbe=true;',[Text.UTF8Encoding]::new($false))
 $env:PRODUCTION_CYCLE_TEST_STARTUP_TIMEOUT_MS='7000'
 Expect-VisibleFailure 'frontend-ready-timeout' 'Превышено время ожидания'
}finally{
 [IO.File]::WriteAllBytes($app,$original)
 $env:PRODUCTION_CYCLE_SMOKE_REPORT=$oldReport;$env:PRODUCTION_CYCLE_TEST_STARTUP_TIMEOUT_MS=$oldTimeout
}
