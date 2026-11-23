Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class M2 {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  public static void RClick(int x,int y){ SetCursorPos(x,y); System.Threading.Thread.Sleep(120); mouse_event(0x0008,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(80); mouse_event(0x0010,0,0,0,UIntPtr.Zero); }
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
$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
Write-Output ("screen " + $b.Width + "x" + $b.Height)
$bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
$g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
$bmp.Save("C:\Windows\System32\_rayclip_tmp\full_before.png",[System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose();$bmp.Dispose()
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driver2"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=700; $script:s=0
$t.Add_Tick({
  $script:s++
  switch($script:s){
    2  { [M2]::RClick(($script:x0+150), ($script:y0+118)) }
    4  { Shot "rc_after" }
    6  { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$script:x0=[int](($b.Width-750)/2); $script:y0=[int](($b.Height-475)*0.18); $t.Start()})
[System.Windows.Forms.Application]::Run($form); Write-Output ok
