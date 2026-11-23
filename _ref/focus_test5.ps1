Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class FB5 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$log = "C:\Windows\System32\_rayclip_tmp\focus_test5.log"
Remove-Item $log -ErrorAction SilentlyContinue
function Say($m) { Add-Content -Path $log -Value $m }
function Click($x, $y) {
  [FB5]::SetCursorPos($x, $y)
  Start-Sleep -Milliseconds 130
  [FB5]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
  [FB5]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}

$h = [FB5]::FindWindow("Tauri Window","Rayclip")
$script:r = New-Object FB5+RECT
$form = New-Object System.Windows.Forms.Form
$form.Width = 180; $form.Height = 60; $form.Text = "f5drv"
$t = New-Object System.Windows.Forms.Timer
$t.Interval = 900
$script:s = 0
$t.Add_Tick({
  $script:s++
  $k = [System.Windows.Forms.SendKeys]
  switch ($script:s) {
    1 { $k::SendWait("^+v") }
    2 {
      [FB5]::GetWindowRect($h, [ref]$script:r) | Out-Null
      Say ("abriu: visivel=" + [FB5]::IsWindowVisible($h) + " rect=" + $script:r.L + "," + $script:r.T + " " + ($script:r.R-$script:r.L) + "x" + ($script:r.B-$script:r.T))
    }
    4 {
      Click ($script:r.L + 620) ($script:r.T + 150)
      Say "cliquei dentro do painel de detalhes"
    }
    6 { Say ("depois do clique dentro: visivel=" + [FB5]::IsWindowVisible($h)) }
    7 {
      $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
      if ($script:r.L -gt $b.Width / 2) { $x = 80 } else { $x = $b.Width - 80 }
      Click $x 80
      Say "cliquei fora"
    }
    9 { Say ("depois do clique fora: visivel=" + [FB5]::IsWindowVisible($h))
        $bmp = New-Object System.Drawing.Bitmap (($script:r.R-$script:r.L)+40),(($script:r.B-$script:r.T)+40)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen(($script:r.L-20), ($script:r.T-20), 0, 0, $bmp.Size)
        $bmp.Save("C:\Windows\System32\_rayclip_tmp\f5_after.png", [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose() }
    10 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({ $t.Start() })
[System.Windows.Forms.Application]::Run($form)
