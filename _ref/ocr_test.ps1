Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function Shot($name) {
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $x0 = [int](($b.Width - 750) / 2); $y0 = [int](($b.Height - 475) * 0.18)
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $crop = $bmp.Clone((New-Object System.Drawing.Rectangle ($x0-20), ($y0-20), 790, 515), $bmp.PixelFormat)
  $crop.Save("C:\Windows\System32\_rayclip_tmp\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose(); $crop.Dispose()
}

$form = New-Object System.Windows.Forms.Form
$form.Width = 200; $form.Height = 80; $form.Text = "driver"
$t = New-Object System.Windows.Forms.Timer
$t.Interval = 550
$script:step = 0
$t.Add_Tick({
  $script:step++
  switch ($script:step) {
    1  { [System.Windows.Forms.SendKeys]::SendWait("^+v") }
    4  { [System.Windows.Forms.SendKeys]::SendWait("zebrafish") }
    7  { Shot "ocr_search" }
    9  { [System.Windows.Forms.SendKeys]::SendWait("^a") }
    10 { [System.Windows.Forms.SendKeys]::SendWait("jellyfin") }
    13 { Shot "ocr_search2" }
    15 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({ $t.Start() })
[System.Windows.Forms.Application]::Run($form)
Write-Output "ok"
