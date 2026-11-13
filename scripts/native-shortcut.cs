// CI fixture: create a real Windows .lnk without WScript's ANSI path conversion.
using System;
using System.Text;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
[ComImport,Guid("00021401-0000-0000-C000-000000000046")] class ShellLinkObject {}
[ComImport,Guid("000214F9-0000-0000-C000-000000000046"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IShellLinkUnicode {
 void GetPath([Out,MarshalAs(UnmanagedType.LPWStr)]StringBuilder path,int count,IntPtr data,uint flags);
 void GetIDList(out IntPtr id);
 void SetIDList(IntPtr id);
 void GetDescription([Out,MarshalAs(UnmanagedType.LPWStr)]StringBuilder value,int count);
 void SetDescription([MarshalAs(UnmanagedType.LPWStr)]string value);
 void GetWorkingDirectory([Out,MarshalAs(UnmanagedType.LPWStr)]StringBuilder value,int count);
 void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)]string value);
 void GetArguments([Out,MarshalAs(UnmanagedType.LPWStr)]StringBuilder value,int count);
 void SetArguments([MarshalAs(UnmanagedType.LPWStr)]string value);
 void GetHotkey(out short value);
 void SetHotkey(short value);
 void GetShowCmd(out int value);
 void SetShowCmd(int value);
 void GetIconLocation([Out,MarshalAs(UnmanagedType.LPWStr)]StringBuilder value,int count,out int index);
 void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)]string value,int index);
 void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)]string value,uint reserved);
 void Resolve(IntPtr window,uint flags);
 void SetPath([MarshalAs(UnmanagedType.LPWStr)]string value);
}
public static class DesktopShortcut {
 public static void Create(string file,string target,string workingDirectory) {
  object item=new ShellLinkObject();
  try {var link=(IShellLinkUnicode)item;link.SetPath(target);link.SetWorkingDirectory(workingDirectory);link.SetShowCmd(1);((IPersistFile)item).Save(file,true);}
  finally {Marshal.FinalReleaseComObject(item);}
 }
}
