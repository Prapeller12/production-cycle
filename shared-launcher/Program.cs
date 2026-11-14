using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Windows.Forms;

// This client only opens the server in a browser. It never starts a database.
internal static class Program
{
    [STAThread]
    private static int Main()
    {
        try
        {
            string root = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
            string script = Path.Combine(root, "Служебные_файлы", "Open-SharedFolder.ps1");
            if (!File.Exists(script))
                throw new InvalidOperationException("Не найдены служебные файлы. Открывайте программу из полной папки, подготовленной администратором.");
            string powershell = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),
                "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
            var info = new ProcessStartInfo(powershell);
            info.Arguments = "-NoLogo -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"" + script + "\"";
            info.UseShellExecute = false;
            info.CreateNoWindow = true;
            using (Process process = Process.Start(info))
            {
                process.WaitForExit();
                return process.ExitCode;
            }
        }
        catch (Exception error)
        {
            MessageBox.Show(error.Message, "Не удалось открыть производственный цикл",
                MessageBoxButtons.OK, MessageBoxIcon.Error);
            return 1;
        }
    }
}
