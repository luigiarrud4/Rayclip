Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\Lugia\AppData\Roaming\com.rayclip.app\clipboard\store.json" -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 700
Start-Process "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3
Set-Clipboard -Value "https://www.raycast.com/changelog"; Start-Sleep -Milliseconds 650
Set-Clipboard -Value "#FF6363"; Start-Sleep -Milliseconds 650
Set-Clipboard -Value "#59D499"; Start-Sleep -Milliseconds 650
Set-Clipboard -Value "rafael@raycast.com"; Start-Sleep -Milliseconds 650
Set-Clipboard -Path "C:\Users\Lugia\Projects\rayclip\package.json"; Start-Sleep -Milliseconds 650
Set-Clipboard -Value "Your shortcut to everything. Raycast is a blazingly fast, totally extendable launcher."; Start-Sleep -Milliseconds 650
$img=[System.Drawing.Image]::FromFile("C:\Users\Lugia\Projects\rayclip\_ref\ocr-sample.png")
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Seconds 4
Write-Output "seeded"
