using System;
using System.IO;
using System.Text;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Imaging;

[ComVisible(true), ClassInterface(ClassInterfaceType.AutoDual)]
public class BrowserReport {
    public Action<string> Receive;
    public Action<string> SaveCapture;
    public Action<int,int> ResizeViewport;
    public Func<int> CountPrintDialogs;
    public Func<string> DescribePrintDialogs;
    public void Report(string json) { Receive(json); }
    public void Capture(string name) { SaveCapture(name); }
    public void Viewport(int width, int height) { ResizeViewport(width,height); }
    public int NativePrintCount() { return CountPrintDialogs(); }
    public string NativePrintDetails() { return DescribePrintDialogs(); }
}
public class CaptureBrowser : WebBrowser {
    [StructLayout(LayoutKind.Sequential)] private struct Rect {public int Left,Top,Right,Bottom;}
    [DllImport("ole32.dll")] private static extern int OleDraw([MarshalAs(UnmanagedType.IUnknown)] object view,uint aspect,IntPtr dc,ref Rect bounds);
    public void SavePng(string path) {
        using(var bitmap=new Bitmap(ClientSize.Width,ClientSize.Height)) {
            using(var graphics=Graphics.FromImage(bitmap)) {
                graphics.Clear(Color.White);IntPtr dc=graphics.GetHdc();
                try {var bounds=new Rect{Left=0,Top=0,Right=bitmap.Width,Bottom=bitmap.Height};Marshal.ThrowExceptionForHR(OleDraw(ActiveXInstance,1,dc,ref bounds));}
                finally {graphics.ReleaseHdc(dc);}
            }
            bitmap.Save(path,ImageFormat.Png);
        }
    }
}
public class IETest {
    private delegate bool EnumWindowProc(IntPtr window,IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowProc callback,IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent,EnumWindowProc callback,IntPtr parameter);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window,StringBuilder text,int count);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window,StringBuilder text,int count);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window,out uint process);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
    [StructLayout(LayoutKind.Sequential)] private struct WindowRect {public int Left,Top,Right,Bottom;}
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr window,out WindowRect rect);
    [DllImport("user32.dll")] private static extern bool PrintWindow(IntPtr window,IntPtr dc,uint flags);
    [STAThread]
    public static int Main(string[] args) {
        if (args.Length != 3) return 2;
        Application.EnableVisualStyles();
        Form form = new Form(); form.AutoScaleMode=AutoScaleMode.None;form.ClientSize = new Size(1440,1000);
        CaptureBrowser browser = new CaptureBrowser(); browser.Dock = DockStyle.None;browser.Size = new Size(1440,1000);
        browser.ScriptErrorsSuppressed = true; form.Controls.Add(browser);
        DateTime deadline = DateTime.UtcNow.AddSeconds(180);
        bool injected = false, finished = false; int code = 1;
        BrowserReport report = new BrowserReport();
        int printDialogs=0;string printDetails="";var seenDialogs=new System.Collections.Generic.HashSet<IntPtr>();
        report.CountPrintDialogs=delegate{return printDialogs;};report.DescribePrintDialogs=delegate{return printDetails;};
        Timer nativeTimer=new Timer();nativeTimer.Interval=100;
        nativeTimer.Tick+=delegate {
            if(Environment.GetEnvironmentVariable("IE_PRINT_DIALOGS")!="1")return;
            var current=new System.Collections.Generic.HashSet<IntPtr>();
            EnumWindows(delegate(IntPtr window,IntPtr parameter){
                uint process;GetWindowThreadProcessId(window,out process);
                if(process!=(uint)System.Diagnostics.Process.GetCurrentProcess().Id)return true;
                var title=new StringBuilder(256);var klass=new StringBuilder(256);GetWindowText(window,title,256);GetClassName(window,klass,256);
                if(klass.ToString()=="#32770"&&title.ToString().Equals("Print",StringComparison.OrdinalIgnoreCase)){
                    current.Add(window);if(seenDialogs.Add(window)){
                        printDialogs++;printDetails+=title.ToString()+" | "+klass.ToString();
                        EnumChildWindows(window,delegate(IntPtr child,IntPtr ignored){var text=new StringBuilder(256);GetWindowText(child,text,256);if(text.Length>0)printDetails+=" | "+text.ToString();return true;},IntPtr.Zero);printDetails+="\n";
                        if(printDialogs==1){
                            WindowRect bounds;if(GetWindowRect(window,out bounds)&&bounds.Right>bounds.Left&&bounds.Bottom>bounds.Top){
                                using(var bitmap=new Bitmap(bounds.Right-bounds.Left,bounds.Bottom-bounds.Top)){
                                    using(var graphics=Graphics.FromImage(bitmap)){IntPtr dc=graphics.GetHdc();try{PrintWindow(window,dc,0);}finally{graphics.ReleaseHdc(dc);}}
                                    bitmap.Save(Path.Combine(Path.GetDirectoryName(args[2]),Path.GetFileNameWithoutExtension(args[2])+"-dialog.png"),ImageFormat.Png);
                                }
                            }
                        }
                    }
                    PostMessage(window,0x111,new IntPtr(2),IntPtr.Zero);
                }
                return true;
            },IntPtr.Zero);
            seenDialogs.IntersectWith(current);
        };nativeTimer.Start();
        report.SaveCapture=delegate(string name){
            if(!System.Text.RegularExpressions.Regex.IsMatch(name,"^[a-z0-9-]+$"))throw new ArgumentException("Invalid capture name");
            browser.SavePng(Path.Combine(Path.GetDirectoryName(args[2]),Path.GetFileNameWithoutExtension(args[2])+"-"+name+".png"));
        };
        report.ResizeViewport=delegate(int width,int height){
            // Size the child directly: the runner desktop can be smaller than
            // the requested render surface. OleDraw captures the whole control.
            form.ClientSize=new Size(width,height);browser.Size=new Size(width,height);
            browser.Update();
        };
        report.Receive = delegate(string json) {
            if (finished) return; finished = true;
            File.WriteAllText(args[2], json, new UTF8Encoding(false));
            code = json.Contains("\"error\":null") ? 0 : 1;
            form.BeginInvoke((Action)delegate { form.Close(); });
        };
        browser.ObjectForScripting = report;
        Timer timer = new Timer(); timer.Interval = 300;
        timer.Tick += delegate {
            if (finished) return;
            if (DateTime.UtcNow > deadline) {
                string detail = "";
                try { detail = browser.Document.Body.InnerText; } catch { }
                detail = detail.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\r", " ").Replace("\n", " ");
                File.WriteAllText(args[2], "{\"error\":\"IE11 timeout: " + detail + "\"}", new UTF8Encoding(false));
                finished = true; form.Close(); return;
            }
            if (!injected && browser.Document != null) {
                try {
                    string probe=Environment.GetEnvironmentVariable("IE_ENTRY_PROBE")=="1"?"document.readyState==='complete'":"!!(window.ProductionServer && window.Production && Production.backend)";
                    object ready = browser.Document.InvokeScript("eval", new object[] { probe });
                    if (ready is bool && (bool)ready) {
                        injected = true;
                        browser.Document.InvokeScript("eval", new object[] { File.ReadAllText(args[1], Encoding.UTF8) });
                    }
                } catch { }
            }
        };
        browser.Navigate(args[0]); timer.Start(); Application.Run(form); timer.Stop();nativeTimer.Stop(); return code;
    }
}
