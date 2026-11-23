Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class N2 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
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
$h=[N2]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driver4"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=620; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1  { $k::SendWait("^+v") }
    2  { if (-not [N2]::IsWindowVisible($h)) { $k::SendWait("^+v") } }
    4  { $k::SendWait("128.45") }
    7  { Shot "t_number" }
    8  { $k::SendWait("^a"); $k::SendWait("zzzq") }
    11 { Shot "t_empty" }
    12 { $k::SendWait("^a"); $k::SendWait("{ESC}"); $k::SendWait("{ESC}") }
    14 { $k::SendWait("^+v") }
    15 { if (-not [N2]::IsWindowVisible($h)) { $k::SendWait("^+v") } }
    17 { $k::SendWait("{DOWN}{DOWN}{DOWN}") }
    19 { $k::SendWait("^p") }
    22 { Shot "t_pinned" }
    23 { $k::SendWait("^k") }
    25 { Shot "t_actions2" }
    26 { $k::SendWait("{ESC}") }
    28 { $k::SendWait("^+{BACKSPACE}") }
    31 { Shot "t_cleared" }
    33 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form); Write-Output ok
