// CI only: remove the runner's administrator group/privileges before GUI launch.
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
public static class LimitedLaunch {
 [StructLayout(LayoutKind.Sequential)] struct SidAndAttributes { public IntPtr Sid; public uint Attributes; }
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StartupInfo { public int cb; public string reserved,desktop,title; public int x,y,xSize,ySize,xChars,yChars,fill,flags; public short show,reserved2; public IntPtr reservedPtr,input,output,error; }
 [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process,thread; public uint pid,tid; }
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr token,uint flags,uint disableCount,ref SidAndAttributes disabled,uint privileges,IntPtr deleted,uint restricted,IntPtr sids,out IntPtr result);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool SetTokenInformation(IntPtr token,int kind,ref SidAndAttributes value,int size);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessAsUserW(IntPtr token,string executable,StringBuilder command,IntPtr processSecurity,IntPtr threadSecurity,bool inherit,uint flags,IntPtr environment,string directory,ref StartupInfo startup,out ProcessInfo process);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 static void Check(bool result) { if(!result)throw new Win32Exception(Marshal.GetLastWin32Error()); }
 static IntPtr Sid(string value) { var sid=new SecurityIdentifier(value);var bytes=new byte[sid.BinaryLength];sid.GetBinaryForm(bytes,0);var ptr=Marshal.AllocHGlobal(bytes.Length);Marshal.Copy(bytes,0,ptr,bytes.Length);return ptr; }
 public static Process Start(string executable,string arguments,string directory) {
  IntPtr original=IntPtr.Zero,limited=IntPtr.Zero,admin=Sid("S-1-5-32-544"),medium=Sid("S-1-16-8192");
  try {
   Check(OpenProcessToken(Process.GetCurrentProcess().Handle,0xF01FF,out original));
   var disabled=new SidAndAttributes{Sid=admin,Attributes=0};
   Check(CreateRestrictedToken(original,1,1,ref disabled,0,IntPtr.Zero,0,IntPtr.Zero,out limited));
   var integrity=new SidAndAttributes{Sid=medium,Attributes=0x20};
   Check(SetTokenInformation(limited,25,ref integrity,Marshal.SizeOf(typeof(SidAndAttributes))+new SecurityIdentifier("S-1-16-8192").BinaryLength));
   using(var identity=new WindowsIdentity(limited)) {
    if(new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator))throw new Exception("CI token still has administrator membership");
   }
   var startup=new StartupInfo{cb=Marshal.SizeOf(typeof(StartupInfo)),desktop=null};ProcessInfo info;
   Check(CreateProcessAsUserW(limited,executable,new StringBuilder("\""+executable+"\" "+arguments),IntPtr.Zero,IntPtr.Zero,false,0,IntPtr.Zero,directory,ref startup,out info));
   try{return Process.GetProcessById((int)info.pid);}finally{CloseHandle(info.thread);CloseHandle(info.process);}
  } finally { if(limited!=IntPtr.Zero)CloseHandle(limited);if(original!=IntPtr.Zero)CloseHandle(original);Marshal.FreeHGlobal(admin);Marshal.FreeHGlobal(medium); }
 }
}
