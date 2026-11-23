Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
# crop around expected window (750x475 centered, 18% from top)
$cx = [int](($b.Width - 750) / 2) - 40
$cy = [int](($b.Height - 475) * 0.18) - 40
$rect = New-Object System.Drawing.Rectangle $cx, $cy, 830, 560
$crop = $bmp.Clone($rect, $bmp.PixelFormat)
$crop.Save("C:\Windows\System32\_rayclip_tmp\shot.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose(); $crop.Dispose()
Write-Output "saved"
