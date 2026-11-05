// CI only: log on as a genuine standard account, with its HKCU profile loaded.
// CreateRestrictedToken cannot represent an ordinary WebView2 user's login:
// MicrosoftEdge/WebView2Feedback#4850.
using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text;
public static class LimitedLaunch {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StartupInfo { public int cb; public string reserved,desktop,title; public int x,y,xSize,ySize,xChars,yChars,fill,flags; public short show,reserved2; public IntPtr reservedPtr,input,output,error; }
 [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process,thread; public uint pid,tid; }
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool LogonUserW(string user,string domain,string password,int kind,int provider,out IntPtr token);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessWithLogonW(string user,string domain,string password,uint logonFlags,string executable,StringBuilder command,uint flags,IntPtr environment,string directory,ref StartupInfo startup,out ProcessInfo process);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
 [DllImport("user32.dll")] static extern IntPtr GetProcessWindowStation();
 [DllImport("user32.dll")] static extern IntPtr GetThreadDesktop(uint thread);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool GetUserObjectInformationW(IntPtr handle,int kind,StringBuilder value,uint bytes,out uint needed);
 [DllImport("user32.dll",SetLastError=true)] static extern bool GetUserObjectSecurity(IntPtr handle,ref uint information,byte[] value,uint bytes,out uint needed);
 [DllImport("user32.dll",SetLastError=true)] static extern bool SetUserObjectSecurity(IntPtr handle,ref uint information,byte[] value);
 static string user,password,desktop;
 static IntPtr stationHandle,desktopHandle;
 static byte[] stationSecurity,desktopSecurity;
 static void Check(bool result) { if(!result)throw new Win32Exception(Marshal.GetLastWin32Error()); }
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
   }
  }finally{if(token!=IntPtr.Zero)CloseHandle(token);}
 }
 public static void Restore() {
  uint info=4;
  if(desktopSecurity!=null)Check(SetUserObjectSecurity(desktopHandle,ref info,desktopSecurity));
  if(stationSecurity!=null)Check(SetUserObjectSecurity(stationHandle,ref info,stationSecurity));
  password=null;user=null;desktopSecurity=null;stationSecurity=null;
 }
 public static Process Start(string executable,string arguments,string directory) {
  if(user==null)throw new Exception("Standard-user CI account is not configured");
  var values=new SortedDictionary<string,string>(StringComparer.OrdinalIgnoreCase);
  foreach(DictionaryEntry item in Environment.GetEnvironmentVariables())values[(string)item.Key]=(string)item.Value;
  var block=new StringBuilder();foreach(var item in values)block.Append(item.Key).Append('=').Append(item.Value).Append('\0');block.Append('\0');
  IntPtr environment=Marshal.StringToHGlobalUni(block.ToString());
  try {
   var startup=new StartupInfo{cb=Marshal.SizeOf(typeof(StartupInfo)),desktop=desktop};ProcessInfo info;
   Check(CreateProcessWithLogonW(user,".",password,1,executable,new StringBuilder("\""+executable+"\" "+arguments),0x400,environment,directory,ref startup,out info));
   try{return Process.GetProcessById((int)info.pid);}finally{CloseHandle(info.thread);CloseHandle(info.process);}
  }finally{Marshal.FreeHGlobal(environment);}
 }
}
