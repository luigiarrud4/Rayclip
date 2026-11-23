Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName Microsoft.VisualBasic
[Microsoft.VisualBasic.Interaction]::AppActivate(34688) | Out-Null
Start-Sleep -Milliseconds 200
[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 900
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$cx = [int](($b.Width - 750) / 2) - 40
$cy = [int](($b.Height - 475) * 0.18) - 40
$rect = New-Object System.Drawing.Rectangle $cx, $cy, 830, 560
$crop = $bmp.Clone($rect, $bmp.PixelFormat)
$crop.Save("C:\Windows\System32\_rayclip_tmp\shot2.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose(); $crop.Dispose()
Write-Output "saved"
