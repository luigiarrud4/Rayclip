Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class ST {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
  public static void Drag(int x1,int y1,int x2,int y2){
    SetCursorPos(x1,y1); System.Threading.Thread.Sleep(200);
    mouse_event(0x0002,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(200);
    for(int i=1;i<=20;i++){ SetCursorPos(x1+(x2-x1)*i/20,y1+(y2-y1)*i/20); System.Threading.Thread.Sleep(30); }
    System.Threading.Thread.Sleep(180); mouse_event(0x0004,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(250);
  }
}
"@
function Shot($n) {
  $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
  $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
  $hook=[ST]::FindWindow("Tauri Window","Rayclip")
  $r=New-Object ST+RECT; [ST]::GetWindowRect($hook,[ref]$r)|Out-Null
  $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($r.L-24),($r.T-24),($($r.R-$r.L)+48),($($r.B-$r.T)+48))),$bmp.PixelFormat)
  $c.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose();$bmp.Dispose();$c.Dispose()
}
$h=[ST]::FindWindow("Tauri Window","Rayclip")
$log="C:\Windows\System32\_rayclip_tmp\status.txt"
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvS"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=650; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [ST]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("{HOME}") }
    6 { $k::SendWait("^c") }
    8 { Shot "s_copied" }
    9 { $r=New-Object ST+RECT; [ST]::GetWindowRect($h,[ref]$r)|Out-Null
        "size: $($r.R-$r.L)x$($r.B-$r.T) at $($r.L),$($r.T)" | Out-File $log -Encoding utf8 }
    10 { $r=New-Object ST+RECT; [ST]::GetWindowRect($h,[ref]$r)|Out-Null
        $script:before="$($r.L),$($r.T)"
        [ST]::Drag(($r.L+430),($r.T+16),($r.L+560),($r.T+96)) }
    14 { $r=New-Object ST+RECT; [ST]::GetWindowRect($h,[ref]$r)|Out-Null
        "after drag: $($r.L),$($r.T) (before $script:before)" | Out-File $log -Append -Encoding utf8
        $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
