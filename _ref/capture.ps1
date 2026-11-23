Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function Shot($name) {
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $cx = [int](($b.Width - 750) / 2) - 26
  $cy = [int](($b.Height - 475) * 0.18) - 26
  $crop = $bmp.Clone((New-Object System.Drawing.Rectangle $cx, $cy, 802, 527), $bmp.PixelFormat)
  $crop.Save("C:\Windows\System32\_rayclip_tmp\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose(); $crop.Dispose()
}

Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 800
Start-Process "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3

$form = New-Object System.Windows.Forms.Form
$form.Width = 300; $form.Height = 120; $form.Text = "capture driver"
$t = New-Object System.Windows.Forms.Timer
$t.Interval = 500
$script:step = 0
$t.Add_Tick({
  $script:step++
  switch ($script:step) {
    1 { [System.Windows.Forms.SendKeys]::SendWait("% ") }
    4 { Shot "final_list" }
    6 { [System.Windows.Forms.SendKeys]::SendWait("^k") }
    9 { Shot "final_actions" }
    11 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({ $t.Start() })
[System.Windows.Forms.Application]::Run($form)
Write-Output "captured"
