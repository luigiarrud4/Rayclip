Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 800
Start-Process "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3
[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 1300
[System.Windows.Forms.SendKeys]::SendWait("^k")
Start-Sleep -Milliseconds 900
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$cx = [int](($b.Width - 750) / 2) - 30
$cy = [int](($b.Height - 475) * 0.18) - 30
$crop = $bmp.Clone((New-Object System.Drawing.Rectangle $cx, $cy, 810, 535), $bmp.PixelFormat)
$crop.Save("C:\Windows\System32\_rayclip_tmp\shot_actions.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose(); $crop.Dispose()
Write-Output "saved"
