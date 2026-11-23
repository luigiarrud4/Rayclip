Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class PP {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public struct RECT { public int L,T,R,B; }
}
"@
$png = "C:\Users\Lugia\Projects\rayclip\_ref\ocr-sample.png"
$img=[System.Drawing.Image]::FromFile($png)
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Seconds 3

Get-Process mspaint -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
$paint = Start-Process mspaint -PassThru
Start-Sleep -Seconds 4

$form=New-Object System.Windows.Forms.Form
$form.Width=60; $form.Height=40; $form.StartPosition="Manual"; $form.Location=New-Object System.Drawing.Point(-3000,-3000)
$t=New-Object System.Windows.Forms.Timer; $t.Interval=800; $script:s=0
$t.Add_Tick({
  $script:s++
  $script:paintHwnd = (Get-Process -Id $script:paintId).MainWindowHandle
  switch($script:s){
    1 { [PP]::SetForegroundWindow($script:paintHwnd) | Out-Null; Start-Sleep -Milliseconds 300
        [System.Windows.Forms.SendKeys]::SendWait("^+v") }
    2 { if(-not [PP]::IsWindowVisible([PP]::FindWindow("Tauri Window","Rayclip"))) {
          [PP]::SetForegroundWindow($script:paintHwnd) | Out-Null; Start-Sleep -Milliseconds 200
          [System.Windows.Forms.SendKeys]::SendWait("^+v") } }
    4 { [System.Windows.Forms.SendKeys]::SendWait("{HOME}") }
    5 { [System.Windows.Forms.SendKeys]::SendWait("{ENTER}") }
    9 { $r=New-Object PP+RECT; [PP]::GetWindowRect($script:paintHwnd,[ref]$r)|Out-Null
        $bmp=New-Object System.Drawing.Bitmap ($r.R-$r.L),($r.B-$r.T)
        $g=[System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($r.L,$r.T,0,0,$bmp.Size)
        $bmp.Save("C:\Windows\System32\_rayclip_tmp\paint.png",[System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose();$bmp.Dispose()
        $t.Stop(); $form.Close() }
  }
})
$script:paintId = $paint.Id
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
Write-Output ("paint pid " + $paint.Id)
