Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName Microsoft.VisualBasic

Start-Process -FilePath "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3

# feed the clipboard with a few different types
$items = @("https://www.raycast.com/changelog", "#FF6363", "rafael@raycast.com",
           "Your shortcut to everything. Raycast is a blazingly fast, totally extendable launcher.")
foreach ($i in $items) { Set-Clipboard -Value $i; Start-Sleep -Milliseconds 650 }

$img = [System.Drawing.Image]::FromFile("C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\frontend\clipboard-history-DUnX9xJX.png")
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Milliseconds 900

[Microsoft.VisualBasic.Interaction]::AppActivate(34688) | Out-Null
Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 1100

$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$cx = [int](($b.Width - 750) / 2) - 30
$cy = [int](($b.Height - 475) * 0.18) - 30
$crop = $bmp.Clone((New-Object System.Drawing.Rectangle $cx, $cy, 810, 535), $bmp.PixelFormat)
$crop.Save("C:\Windows\System32\_rayclip_tmp\shot3.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose(); $crop.Dispose()
Write-Output "saved"
