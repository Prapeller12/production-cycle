$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()
$f=New-Object Windows.Forms.Form
$f.Text='Производственный цикл — ключ базы'
$f.StartPosition='CenterScreen'
$f.TopMost=$true
$f.FormBorderStyle='FixedDialog'
$f.MaximizeBox=$false
$f.MinimizeBox=$false
$f.ClientSize=New-Object Drawing.Size(525,245)
function LabelAt($t,$x,$y,$w,$h){
  $l=New-Object Windows.Forms.Label
  $l.Text=$t;$l.SetBounds($x,$y,$w,$h);$f.Controls.Add($l)
}
LabelAt 'Введите парольную фразу базы. При первом запуске задайте новую и сохраните её отдельно. Для общей базы все компьютеры используют одну фразу. Персональный ПИН вводится позже.' 18 15 488 52
LabelAt 'Парольная фраза хранилища (от 12 символов)' 18 73 490 20
$a=New-Object Windows.Forms.TextBox;$a.UseSystemPasswordChar=$true;$a.SetBounds(18,93,488,23);$f.Controls.Add($a)
LabelAt 'Повторите фразу' 18 124 490 19
$b=New-Object Windows.Forms.TextBox;$b.UseSystemPasswordChar=$true;$b.SetBounds(18,145,488,23);$f.Controls.Add($b)
$ok=New-Object Windows.Forms.Button;$ok.Text='Открыть';$ok.SetBounds(395,196,111,30);$f.Controls.Add($ok);$f.AcceptButton=$ok
$ok.Add_Click({if($a.Text.Length -lt 12 -or $a.Text -cne $b.Text){[void][Windows.Forms.MessageBox]::Show($f,'Проверьте длину и совпадение фраз.');return};$f.DialogResult='OK'})
if($f.ShowDialog() -ne 'OK'){exit 2}
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::Write($a.Text)
$a.Clear();$b.Clear();$f.Dispose()
