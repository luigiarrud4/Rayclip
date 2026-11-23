Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class WC {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
  public static void Drag(int x1,int y1,int x2,int y2){
    SetCursorPos(x1,y1); System.Threading.Thread.Sleep(180);
    mouse_event(0x0002,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(180);
    int steps=18;
    for(int i=1;i<=steps;i++){ SetCursorPos(x1+(x2-x1)*i/steps, y1+(y2-y1)*i/steps); System.Threading.Thread.Sleep(28); }
    System.Threading.Thread.Sleep(160); mouse_event(0x0004,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(220);
  }
}
"@
$h=[WC]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driverW"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=700; $script:s=0
$t.Add_Tick({
  $script:s++
  switch($script:s){
    1 { [System.Windows.Forms.SendKeys]::SendWait("^+v") }
    2 { if(-not [WC]::IsWindowVisible($h)){ [System.Windows.Forms.SendKeys]::SendWait("^+v") } }
    4 { $r=New-Object WC+RECT; [WC]::GetWindowRect($h,[ref]$r)|Out-Null
        Write-Output ("before: " + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))
      }
    5 { $r=New-Object WC+RECT; [WC]::GetWindowRect($h,[ref]$r)|Out-Null
        [WC]::Drag(($r.L+300),($r.T+20),($r.L+300+140),($r.T+20+90)) }   # arrasta pelo header
    9 { $r=New-Object WC+RECT; [WC]::GetWindowRect($h,[ref]$r)|Out-Null
        Write-Output ("after drag: " + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))
        [WC]::Drag(($r.R-3),($r.B-3),($r.R+167),($r.B+125)) }             # redimensiona pelo canto
    13 { $r=New-Object WC+RECT; [WC]::GetWindowRect($h,[ref]$r)|Out-Null
         Write-Output ("after resize: " + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T)) }
    15 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
