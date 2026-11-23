Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Text; using System.Runtime.InteropServices;
public class N {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
  public static void RClick(int x,int y){ SetCursorPos(x,y); System.Threading.Thread.Sleep(140); mouse_event(0x0008,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(90); mouse_event(0x0010,0,0,0,UIntPtr.Zero); }
}
"@
function Shot($n) {
  $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $x0=[int](($b.Width-750)/2); $y0=[int](($b.Height-475)*0.18)
  $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
  $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
  $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($x0-24),($y0-24),798,523)),$bmp.PixelFormat)
  $c.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose();$bmp.Dispose();$c.Dispose()
}
$h=[N]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driver3"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=650; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if (-not [N]::IsWindowVisible($h)) { $k::SendWait("^+v") } }
    3 { Write-Output ("visible=" + [N]::IsWindowVisible($h)) }
    4 { Shot "rc_before" }
    5 { $r=New-Object N+RECT; [N]::GetWindowRect($h,[ref]$r)|Out-Null; [N]::RClick(($r.L+150),($r.T+118)) }
    8 { Shot "rc_after" }
    10 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form); Write-Output ok
