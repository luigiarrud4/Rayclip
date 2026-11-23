Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\Lugia\AppData\Roaming\com.rayclip.app\clipboard\store.json" -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 600
Start-Process "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3

Set-Clipboard -Value "Your shortcut to everything. Raycast is a blazingly fast, totally extendable launcher."
Start-Sleep -Milliseconds 650
Set-Clipboard -Value "https://www.raycast.com/changelog"
Start-Sleep -Milliseconds 650
Set-Clipboard -Value "#FF6363"
Start-Sleep -Milliseconds 650
Set-Clipboard -Value "#59D499"
Start-Sleep -Milliseconds 650
Set-Clipboard -Value "rafael@raycast.com"
Start-Sleep -Milliseconds 650
Set-Clipboard -Path "C:\Users\Lugia\Projects\rayclip\package.json"
Start-Sleep -Milliseconds 650
$img = [System.Drawing.Image]::FromFile("C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\frontend\clipboard-history-DUnX9xJX.png")
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Milliseconds 900

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

[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 1400
Shot "final_list"
[System.Windows.Forms.SendKeys]::SendWait("^k")
Start-Sleep -Milliseconds 800
Shot "final_actions"
Write-Output "done"
