Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class OS2 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@
$h=[OS2]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvO"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=900; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [OS2]::IsWindowVisible($h)){ [OS2]::SetForegroundWindow($h); $k::SendWait("^+v") } }
    4 { [OS2]::SetForegroundWindow($h); Start-Sleep -Milliseconds 200; $k::SendWait("^,") }
    6 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
