Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class E2 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
}
"@
$img=[System.Drawing.Image]::FromFile("C:\Users\Lugia\Projects\rayclip\_ref\ocr-sample.png")
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Seconds 4
$dir="C:\Users\Lugia\AppData\Roaming\com.rayclip.app\clipboard\images"
$newest = Get-ChildItem $dir -Filter *.png | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Move-Item $newest.FullName ($newest.FullName + ".gone") -Force
Write-Output ("quebrei: " + $newest.Name)
$h=[E2]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvE2"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=650; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [E2]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("{HOME}") }
    6 { $k::SendWait("^c") }
    9 { $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $r=New-Object E2+RECT; [E2]::GetWindowRect($h,[ref]$r)|Out-Null
        $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
        $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
        $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($r.L-24),($r.T-24),($($r.R-$r.L)+48),($($r.B-$r.T)+48))),$bmp.PixelFormat)
        $c.Save("C:\Windows\System32\_rayclip_tmp\s_error.png",[System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose();$bmp.Dispose();$c.Dispose()
        $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
