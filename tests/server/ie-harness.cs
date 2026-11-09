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
    public void Report(string json) { Receive(json); }
    public void Capture(string name) { SaveCapture(name); }
    public void Viewport(int width, int height) { ResizeViewport(width,height); }
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
    [DllImport("user32.dll",SetLastError=true)] private static extern bool SetWindowPos(IntPtr window,IntPtr after,int x,int y,int width,int height,uint flags);
    [STAThread]
    public static int Main(string[] args) {
        if (args.Length != 3) return 2;
        Application.EnableVisualStyles();
        Form form = new Form(); form.AutoScaleMode=AutoScaleMode.None;form.ClientSize = new Size(1440,1000);
        CaptureBrowser browser = new CaptureBrowser(); browser.Dock = DockStyle.Fill;
        browser.ScriptErrorsSuppressed = true; form.Controls.Add(browser);
        DateTime deadline = DateTime.UtcNow.AddSeconds(180);
        bool injected = false, finished = false; int code = 1;
        BrowserReport report = new BrowserReport();
        report.SaveCapture=delegate(string name){
            if(!System.Text.RegularExpressions.Regex.IsMatch(name,"^[a-z0-9-]+$"))throw new ArgumentException("Invalid capture name");
            browser.SavePng(Path.Combine(Path.GetDirectoryName(args[2]),Path.GetFileNameWithoutExtension(args[2])+"-"+name+".png"));
        };
        report.ResizeViewport=delegate(int width,int height){
            // Form.ClientSize clamps large windows to the runner's desktop size.
            // Native bounds allow the MSHTML control to render the requested viewport.
            if(!SetWindowPos(form.Handle,IntPtr.Zero,0,0,width+form.Width-form.ClientSize.Width,height+form.Height-form.ClientSize.Height,0x14))throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
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
                    object ready = browser.Document.InvokeScript("eval", new object[] { "!!(window.ProductionServer && window.Production && Production.backend)" });
                    if (ready is bool && (bool)ready) {
                        injected = true;
                        browser.Document.InvokeScript("eval", new object[] { File.ReadAllText(args[1], Encoding.UTF8) });
                    }
                } catch { }
            }
        };
        browser.Navigate(args[0]); timer.Start(); Application.Run(form); timer.Stop(); return code;
    }
}
