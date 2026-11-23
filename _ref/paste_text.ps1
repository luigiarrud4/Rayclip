Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class PT {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
}
"@
$payload = "RAYCLIP-TEXT-" + (Get-Random) + "-OK"
Set-Clipboard -Value $payload
Start-Sleep -Seconds 2
$h=[PT]::FindWindow("Tauri Window","Rayclip")

$form=New-Object System.Windows.Forms.Form
$form.Width=420; $form.Height=200; $form.Text="target"
$tb=New-Object System.Windows.Forms.TextBox
$tb.Multiline=$true; $tb.Dock="Fill"
$form.Controls.Add($tb)
$t=New-Object System.Windows.Forms.Timer; $t.Interval=650; $script:s=0
$t.Add_Tick({
  $script:s++
  switch($script:s){
    1 { $tb.Focus(); [System.Windows.Forms.SendKeys]::SendWait("^+v") }
    2 { if(-not [PT]::IsWindowVisible($h)){ [System.Windows.Forms.SendKeys]::SendWait("^+v") } }
    4 { [System.Windows.Forms.SendKeys]::SendWait("{HOME}") }
    5 { [System.Windows.Forms.SendKeys]::SendWait("{ENTER}") }
    9 { "expected=$payload" | Out-File C:\Windows\System32\_rayclip_tmp\paste_text.txt -Encoding utf8
        ("got=" + $tb.Text) | Out-File C:\Windows\System32\_rayclip_tmp\paste_text.txt -Append -Encoding utf8
        $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
