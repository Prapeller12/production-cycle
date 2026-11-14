// CI only: log on as a genuine standard account, with its HKCU profile loaded.
// CreateRestrictedToken cannot represent an ordinary WebView2 user's login:
// MicrosoftEdge/WebView2Feedback#4850.
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text;
using Microsoft.Win32.SafeHandles;
public static class LimitedLaunch {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StartupInfo { public int cb; public string reserved,desktop,title; public int x,y,xSize,ySize,xChars,yChars,fill,flags; public short show,reserved2; public IntPtr reservedPtr,input,output,error; }
 [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process,thread; public uint pid,tid; }
 [StructLayout(LayoutKind.Sequential)] struct UnicodeString { public ushort length,maximum; public IntPtr buffer; }
 [StructLayout(LayoutKind.Sequential)] struct ObjectAttributes { public int length; public IntPtr root,name; public uint attributes; public IntPtr security,quality; }
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool LogonUserW(string user,string domain,string password,int kind,int provider,out IntPtr token);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessWithLogonW(string user,string domain,string password,uint logonFlags,string executable,StringBuilder command,uint flags,IntPtr environment,string directory,ref StartupInfo startup,out ProcessInfo process);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateMutexW(IntPtr security,bool owned,string name);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr OpenMutexW(uint access,bool inherit,string name);
 [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
 [DllImport("user32.dll")] static extern IntPtr GetProcessWindowStation();
 [DllImport("user32.dll")] static extern IntPtr GetThreadDesktop(uint thread);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool GetUserObjectInformationW(IntPtr handle,int kind,StringBuilder value,uint bytes,out uint needed);
 [DllImport("user32.dll",SetLastError=true)] static extern bool GetUserObjectSecurity(IntPtr handle,ref uint information,byte[] value,uint bytes,out uint needed);
 [DllImport("user32.dll",SetLastError=true)] static extern bool SetUserObjectSecurity(IntPtr handle,ref uint information,byte[] value);
 [DllImport("ntdll.dll")] static extern int NtOpenDirectoryObject(out IntPtr handle,uint access,ref ObjectAttributes attributes);
 [DllImport("ntdll.dll")] static extern uint RtlNtStatusToDosError(int status);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool GetKernelObjectSecurity(IntPtr handle,uint information,byte[] value,uint bytes,out uint needed);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool SetKernelObjectSecurity(IntPtr handle,uint information,byte[] value);
 static string user,password,desktop;
 static IntPtr stationHandle,desktopHandle,namedObjectsHandle;
 static byte[] stationSecurity,desktopSecurity,namedObjectsSecurity;
 static void Check(bool result) { if(!result)throw new Win32Exception(Marshal.GetLastWin32Error()); }
 public static string ProbeNamedObjects() {
  var unique=CreateMutexW(IntPtr.Zero,false,"Local\\PcLaunchProbe-"+Guid.NewGuid().ToString("N"));var createError=Marshal.GetLastWin32Error();if(unique!=IntPtr.Zero)CloseHandle(unique);
  var chrome=OpenMutexW(0x1F0001,false,"Local\\ChromeProcessSingletonStartup!");var openError=Marshal.GetLastWin32Error();if(chrome!=IntPtr.Zero)CloseHandle(chrome);
  return "Create unique local mutex: "+(unique!=IntPtr.Zero)+", Win32 error: "+createError+Environment.NewLine+"Open Chromium startup mutex: "+(chrome!=IntPtr.Zero)+", Win32 error: "+openError;
 }
 static string Name(IntPtr handle) { var text=new StringBuilder(256);uint needed;Check(GetUserObjectInformationW(handle,2,text,512,out needed));return text.ToString(); }
 static byte[] Grant(IntPtr handle,SecurityIdentifier sid,int mask) {
  uint info=4,needed;GetUserObjectSecurity(handle,ref info,null,0,out needed);
  var saved=new byte[needed];Check(GetUserObjectSecurity(handle,ref info,saved,needed,out needed));
  var descriptor=new RawSecurityDescriptor(saved,0);
  var acl=descriptor.DiscretionaryAcl;
  if(acl==null)throw new Exception("CI desktop has no DACL");
  acl.InsertAce(acl.Count,new CommonAce(AceFlags.None,AceQualifier.AccessAllowed,mask,sid,false,null));
  var updated=new byte[descriptor.BinaryLength];descriptor.GetBinaryForm(updated,0);
  Check(SetUserObjectSecurity(handle,ref info,updated));return saved;
 }
 static IntPtr OpenNamedObjects() {
  var name="\\Sessions\\"+Process.GetCurrentProcess().SessionId+"\\BaseNamedObjects";
  var text=Marshal.StringToHGlobalUni(name);var ptr=Marshal.AllocHGlobal(Marshal.SizeOf(typeof(UnicodeString)));
  try {
   var value=new UnicodeString{length=(ushort)(name.Length*2),maximum=(ushort)((name.Length+1)*2),buffer=text};Marshal.StructureToPtr(value,ptr,false);
   var attributes=new ObjectAttributes{length=Marshal.SizeOf(typeof(ObjectAttributes)),name=ptr,attributes=0x40};IntPtr handle;
   int status=NtOpenDirectoryObject(out handle,0x60000,ref attributes);if(status<0)throw new Win32Exception((int)RtlNtStatusToDosError(status),"Cannot prepare the CI session's named-object directory");return handle;
  }finally{Marshal.FreeHGlobal(ptr);Marshal.FreeHGlobal(text);}
 }
 static byte[] GrantNamedObjects(IntPtr handle,SecurityIdentifier sid) {
  uint needed;GetKernelObjectSecurity(handle,4,null,0,out needed);var saved=new byte[needed];Check(GetKernelObjectSecurity(handle,4,saved,needed,out needed));
  var descriptor=new RawSecurityDescriptor(saved,0);var acl=descriptor.DiscretionaryAcl;if(acl==null)throw new Exception("CI named-object directory has no DACL");
  // Query/traverse/create object/create subdirectory. No token privileges or
  // administrator membership are added to the standard test account.
  acl.InsertAce(acl.Count,new CommonAce(AceFlags.None,AceQualifier.AccessAllowed,0xF,sid,false,null));
  var updated=new byte[descriptor.BinaryLength];descriptor.GetBinaryForm(updated,0);Check(SetKernelObjectSecurity(handle,4,updated));return saved;
 }
 public static void Configure(string account,string secret) {
  user=account;password=secret;IntPtr token=IntPtr.Zero;
  try {
   Check(LogonUserW(user,".",password,2,0,out token));
   using(var identity=new WindowsIdentity(token)) {
    if(new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator))throw new Exception("CI test account must not be an administrator");
    stationHandle=GetProcessWindowStation();desktopHandle=GetThreadDesktop(GetCurrentThreadId());
    desktop=Name(stationHandle)+"\\"+Name(desktopHandle);
    stationSecurity=Grant(stationHandle,identity.User,0xF037F);
    desktopSecurity=Grant(desktopHandle,identity.User,0xF01FF);
    // A second logon in the runner's existing interactive session does not own
    // that session's Local namespace. Prepare the same create rights it would
    // receive in its own normal Windows logon session; restore after the smoke.
    namedObjectsHandle=OpenNamedObjects();namedObjectsSecurity=GrantNamedObjects(namedObjectsHandle,identity.User);
   }
  }finally{if(token!=IntPtr.Zero)CloseHandle(token);}
 }
 public static void Restore() {
  uint info=4;
  if(namedObjectsSecurity!=null)Check(SetKernelObjectSecurity(namedObjectsHandle,4,namedObjectsSecurity));
  if(namedObjectsHandle!=IntPtr.Zero)CloseHandle(namedObjectsHandle);
  if(desktopSecurity!=null)Check(SetUserObjectSecurity(desktopHandle,ref info,desktopSecurity));
  if(stationSecurity!=null)Check(SetUserObjectSecurity(stationHandle,ref info,stationSecurity));
  password=null;user=null;desktopSecurity=null;stationSecurity=null;namedObjectsSecurity=null;namedObjectsHandle=IntPtr.Zero;
 }
 public static StartedProcess Start(string executable,string arguments,string directory) {
  if(user==null)throw new Exception("Standard-user CI account is not configured");
  if(arguments!="--smoke-test")throw new Exception("Standard-user helper only runs the native smoke");
  var report=Environment.GetEnvironmentVariable("PRODUCTION_CYCLE_SMOKE_REPORT");
  var folder=Path.GetDirectoryName(report);var bootstrap=Path.Combine(folder,"network-standard-user-bootstrap.cmd");
  var log=Path.Combine(folder,"network-standard-user-webview.log");
  var probe=Path.Combine(folder,"network-standard-user-probe.cs").Replace("'","''");
  var probeReport=Path.Combine(folder,"network-standard-user-probe.txt").Replace("'","''");
  // NULL environment is essential: APPDATA/USERPROFILE/USERNAME and HKCU must
  // belong to this user, not the runner administrator. A batch keeps the loaded
  // profile alive and waits for the GUI EXE; it adds only CI report destinations.
  var batch="@echo off\r\nchcp 65001 >nul\r\n"+
   "set \"PRODUCTION_CYCLE_SMOKE_REPORT="+report+"\"\r\n"+
   "set \"PRODUCTION_CYCLE_STARTUP_REPORT="+Environment.GetEnvironmentVariable("PRODUCTION_CYCLE_STARTUP_REPORT")+"\"\r\n"+
   "set \"WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--enable-logging --log-file=\""+log+"\" --v=1\"\r\n"+
   "whoami /all > \""+Path.Combine(folder,"network-standard-user-identity.txt")+"\"\r\n"+
   "\""+executable+"\" --smoke-test\r\nset PC_SMOKE_EXIT=%errorlevel%\r\n"+
   "powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"Add-Type -Path '"+probe+"'; [LimitedLaunch]::ProbeNamedObjects() | Set-Content -LiteralPath '"+probeReport+"'\"\r\n"+
   "exit /b %PC_SMOKE_EXIT%\r\n";
  File.WriteAllText(bootstrap,batch,new UTF8Encoding(false));
  var shell=Path.Combine(Environment.GetEnvironmentVariable("SystemRoot"),"System32","cmd.exe");
  var startup=new StartupInfo{cb=Marshal.SizeOf(typeof(StartupInfo)),desktop=desktop};ProcessInfo info;
  Check(CreateProcessWithLogonW(user,".",password,1,shell,new StringBuilder("\""+shell+"\" /d /c \"\""+bootstrap+"\"\""),0,IntPtr.Zero,Environment.GetEnvironmentVariable("SystemRoot"),ref startup,out info));
  try{return new StartedProcess(info.process,(int)info.pid);}finally{CloseHandle(info.thread);}
 }
}
// CreateProcessWithLogonW returns a full-access handle. Keep it: reopening an
// already exiting process owned by the other account can lose its exit code.
public sealed class StartedProcess : IDisposable {
 readonly SafeProcessHandle handle;
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(SafeProcessHandle process,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(SafeProcessHandle process,out uint code);
 public int Id { get; private set; }
 public StartedProcess(IntPtr process,int id){handle=new SafeProcessHandle(process,true);Id=id;}
 public bool HasExited { get { var result=WaitForSingleObject(handle,0);if(result==0xFFFFFFFF)throw new Win32Exception(Marshal.GetLastWin32Error());return result==0; } }
 public int ExitCode { get { uint code;if(!GetExitCodeProcess(handle,out code))throw new Win32Exception(Marshal.GetLastWin32Error());return unchecked((int)code); } }
 public void Refresh() {}
 public void WaitForExit(){if(WaitForSingleObject(handle,0xFFFFFFFF)!=0)throw new Win32Exception(Marshal.GetLastWin32Error());}
 public void Dispose(){handle.Dispose();}
}
