Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class FD {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
"@
$log="C:\Windows\System32\_rayclip_tmp\focus.txt"
$paint=(Get-Process mspaint -ErrorAction SilentlyContinue)
if(-not $paint){ $paint = Start-Process mspaint -PassThru; Start-Sleep -Seconds 4 }
$ph=(Get-Process -Id $paint.Id).MainWindowHandle
[FD]::SetForegroundWindow($ph) | Out-Null
Start-Sleep -Milliseconds 300
"paint hwnd=$ph fg=" + [FD]::GetForegroundWindow() | Out-File $log -Encoding utf8
$form=New-Object System.Windows.Forms.Form; $form.Width=40; $form.Height=30
$form.StartPosition="Manual"; $form.Location=New-Object System.Drawing.Point(-3000,-3000)
$t=New-Object System.Windows.Forms.Timer; $t.Interval=500; $script:s=0
$t.Add_Tick({
  $script:s++
  $rail=[FD]::FindWindow("Tauri Window","Rayclip")
  switch($script:s){
    1 { [System.Windows.Forms.SendKeys]::SendWait("^+v") }
    2 { "t+500ms visible=" + [FD]::IsWindowVisible($rail) + " fg=" + [FD]::GetForegroundWindow() + " rail=$rail" | Out-File $log -Append -Encoding utf8 }
    3 { "t+1000ms visible=" + [FD]::IsWindowVisible($rail) + " fg=" + [FD]::GetForegroundWindow() | Out-File $log -Append -Encoding utf8 }
    5 { "t+2s visible=" + [FD]::IsWindowVisible($rail) + " fg=" + [FD]::GetForegroundWindow() | Out-File $log -Append -Encoding utf8
        $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
