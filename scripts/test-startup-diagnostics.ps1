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
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr SendMessageTimeoutW(IntPtr w,uint message,UIntPtr wp,StringBuilder text,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr w,uint msg,IntPtr wp,IntPtr lp);
 public static IntPtr Find(int pid) {IntPtr result=IntPtr.Zero;EnumWindows((w,a)=>{uint owner;GetWindowThreadProcessId(w,out owner);var text=new StringBuilder(256);GetWindowTextW(w,text,256);if(owner==pid&&IsWindowVisible(w)&&text.ToString()=="Производственный цикл — ошибка запуска"){result=w;return false;}return true;},IntPtr.Zero);return result;}
 // The parent becomes visible before EDIT 1001 is created. Also, GetWindowText
 // cannot read an EDIT owned by another process; use marshalled WM_GETTEXT.
 public static string Read(IntPtr w){
  var edit=GetDlgItem(w,1001);if(!IsWindowVisible(w)||edit==IntPtr.Zero||!IsWindowVisible(edit))return null;
  var text=new StringBuilder(32768);UIntPtr copied;
  // SMTO_BLOCK | SMTO_ABORTIFHUNG bounds a probe even if the target stops pumping.
  if(SendMessageTimeoutW(edit,0x000D,new UIntPtr((uint)text.Capacity),text,0x0001|0x0002,1000,out copied)==IntPtr.Zero)return null;
  return text.ToString();
 }
}
'@
$oldReport=$env:PRODUCTION_CYCLE_SMOKE_REPORT;$oldTimeout=$env:PRODUCTION_CYCLE_TEST_STARTUP_TIMEOUT_MS
function Expect-VisibleFailure([string]$name,[string]$pattern){
 $report=Join-Path $out ($name+'.json');$env:PRODUCTION_CYCLE_SMOKE_REPORT=$report
 $process=Start-Process -FilePath "$Root/production-cycle.exe" -ArgumentList '--smoke-test' -WorkingDirectory "$env:SystemRoot/System32" -PassThru
 $deadline=[DateTime]::UtcNow.AddSeconds(40);$dialog=[IntPtr]::Zero;$text='';$complete=$false
 try{
  # Window visibility alone is not readiness: wait for its visible native edit
  # control and all required text. The report file is never a text substitute.
  while([DateTime]::UtcNow -lt $deadline -and -not $process.HasExited){
   $found=[StartupProbe]::Find($process.Id)
   if($found -ne [IntPtr]::Zero){
    $dialog=$found;$observed=[StartupProbe]::Read($dialog)
    if(-not [String]::IsNullOrWhiteSpace($observed)){$text=$observed}
    if($observed -match $pattern -and $observed -match 'Этап:' -and $observed -match 'EXE:' -and $observed -match 'Рабочая папка ярлыка:' -and $observed -match 'доступная локальная папка клиента\.\s*$'){$complete=$true;break}
   }
   Start-Sleep -Milliseconds 100
  }
  if($dialog -eq [IntPtr]::Zero){throw "No visible diagnostic: $name"}
  $text | Set-Content (Join-Path $out ($name+'-visible.txt')) -Encoding utf8
  if(-not $complete){throw "Incomplete visible diagnostic $name : $text"}
  [StartupProbe]::PostMessageW($dialog,0x0111,[IntPtr]1,[IntPtr]::Zero)|Out-Null
  if(-not $process.WaitForExit(15000)){throw "Diagnostic cannot close: $name"}
  $value=Get-Content $report -Raw|ConvertFrom-Json
  if($value.ok -or $process.ExitCode -eq 0){throw "Failure reported success: $name"}
  Write-Host "PASS visible native diagnostic: $name"
 }finally{
  if(-not $process.HasExited){& taskkill.exe /PID $process.Id /T /F 2>$null|Out-Null}
 }
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
