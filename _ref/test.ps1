Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

Start-Process -FilePath "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 4

$items = @(
  "https://www.raycast.com/changelog",
  "#FF6363",
  "rafael@raycast.com",
  "Your shortcut to everything. Raycast is a blazingly fast, totally extendable launcher."
)
foreach ($i in $items) {
  Set-Clipboard -Value $i
  Start-Sleep -Milliseconds 700
}

$img = [System.Drawing.Image]::FromFile("C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\frontend\clipboard-history-DUnX9xJX.png")
[System.Windows.Forms.Clipboard]::SetImage($img)
$img.Dispose()
Start-Sleep -Milliseconds 900

$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
Write-Output "screen $($b.Width)x$($b.Height)"
