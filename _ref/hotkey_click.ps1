param([int]$dx=30,[int]$dy=28,[int]$hot=1)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class HK {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[HK]::FindWindow("Tauri Window","Rayclip")
$script:r=New-Object HK+RECT
function Grab($n){
  [HK]::GetWindowRect($h,[ref]$script:r)|Out-Null
  $bmp=New-Object System.Drawing.Bitmap (($script:r.R-$script:r.L)+40),(($script:r.B-$script:r.T)+40)
  $g=[System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(($script:r.L-20),($script:r.T-20),0,0,$bmp.Size)
  $bmp.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}
$form=New-Object System.Windows.Forms.Form; $form.Width=180; $form.Height=60; $form.Text="hkdrv"
$script:s=0
$t=New-Object System.Windows.Forms.Timer; $t.Interval=900
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [HK]::IsWindowVisible($h)) { $k::SendWait("^+v") } }
    3 { [HK]::SetCursorPos(($script:r.L+$dx),($script:r.B-$dy)); Start-Sleep -Milliseconds 200
        [HK]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [HK]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero) }
    5 { Grab "hk_click" }
    6 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
Write-Output ("visivel no fim: " + [HK]::IsWindowVisible($h))
