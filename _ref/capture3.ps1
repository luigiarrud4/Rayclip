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

Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\Lugia\AppData\Roaming\com.rayclip.app\clipboard\store.json" -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 700
Start-Process "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3

Set-Clipboard -Value "https://www.raycast.com/changelog"
Start-Sleep -Milliseconds 700
Set-Clipboard -Value "#FF6363"
Start-Sleep -Milliseconds 700
Set-Clipboard -Value "#59D499"
Start-Sleep -Milliseconds 700
Set-Clipboard -Value "rafael@raycast.com"
Start-Sleep -Milliseconds 700
Set-Clipboard -Path "C:\Users\Lugia\Projects\rayclip\package.json"
Start-Sleep -Milliseconds 700
Set-Clipboard -Value "Your shortcut to everything. Raycast is a blazingly fast, totally extendable launcher."
Start-Sleep -Milliseconds 700
$img = [System.Drawing.Image]::FromFile("C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\frontend\clipboard-history-DUnX9xJX.png")
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Milliseconds 900

$form = New-Object System.Windows.Forms.Form
$form.Width = 240; $form.Height = 90; $form.Text = "driver"
$t = New-Object System.Windows.Forms.Timer
$t.Interval = 600
$script:step = 0
$t.Add_Tick({
  $script:step++
  switch ($script:step) {
    1 { [System.Windows.Forms.SendKeys]::SendWait("% ") }
    4 { Shot "v_image" }
    5 { [System.Windows.Forms.SendKeys]::SendWait("{DOWN}") }
    6 { [System.Windows.Forms.SendKeys]::SendWait("{DOWN}") }
    7 { [System.Windows.Forms.SendKeys]::SendWait("{DOWN}") }
    8 { [System.Windows.Forms.SendKeys]::SendWait("{DOWN}") }
    9 { Shot "v_file" }
    11 { [System.Windows.Forms.SendKeys]::SendWait("^k") }
    13 { Shot "v_actions" }
    15 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({ $t.Start() })
[System.Windows.Forms.Application]::Run($form)
Write-Output "ok"
