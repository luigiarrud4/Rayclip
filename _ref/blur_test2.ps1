Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class B2 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[B2]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=180; $form.Height=60; $form.Text="blurdrv"
Remove-Item "C:\Windows\System32\_rayclip_tmplur2.log" -ErrorAction SilentlyContinue
$script:s=0; $script:r=New-Object B2+RECT
$t=New-Object System.Windows.Forms.Timer; $t.Interval=900
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { Add-Content -Path "C:\Windows\System32\_rayclip_tmplur2.log" -Value ("depois de mostrar: visivel=" + [B2]::IsWindowVisible($h)); [B2]::GetWindowRect($h,[ref]$script:r)|Out-Null }
    3 { $x = if($script:r.L -gt 900){60}else{1800}
        [B2]::SetCursorPos($x,60); Start-Sleep -Milliseconds 200
        [B2]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [B2]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
        Write-Output "clique fora enviado" }
    5 { Add-Content -Path "C:\Windows\System32\_rayclip_tmplur2.log" -Value ("depois do clique fora: visivel=" + [B2]::IsWindowVisible($h))
        $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
        $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
        $bmp.Save("C:\Windows\System32\_rayclip_tmp\blur2.png",[System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose();$bmp.Dispose() }
    6 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
