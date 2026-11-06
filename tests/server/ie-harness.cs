using System;
using System.IO;
using System.Text;
using System.Runtime.InteropServices;
using System.Windows.Forms;

[ComVisible(true), ClassInterface(ClassInterfaceType.AutoDual)]
public class BrowserReport {
    public Action<string> Receive;
    public void Report(string json) { Receive(json); }
}
public class IETest {
    [STAThread]
    public static int Main(string[] args) {
        if (args.Length != 3) return 2;
        Application.EnableVisualStyles();
        Form form = new Form(); form.Width = 1440; form.Height = 1000;
        WebBrowser browser = new WebBrowser(); browser.Dock = DockStyle.Fill;
        browser.ScriptErrorsSuppressed = true; form.Controls.Add(browser);
        DateTime deadline = DateTime.UtcNow.AddSeconds(180);
        bool injected = false, finished = false; int code = 1;
        BrowserReport report = new BrowserReport();
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
