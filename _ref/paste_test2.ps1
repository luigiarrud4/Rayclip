Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName Microsoft.VisualBasic

$np = Start-Process notepad -PassThru
Start-Sleep -Seconds 3
Write-Output "notepad pid=$($np.Id)"
[Microsoft.VisualBasic.Interaction]::AppActivate($np.Id) | Out-Null
Start-Sleep -Milliseconds 500

Set-Clipboard -Value "HELLO-RAYCLIP-PASTE-TEST-123"
Start-Sleep -Milliseconds 900
[Microsoft.VisualBasic.Interaction]::AppActivate($np.Id) | Out-Null
Start-Sleep -Milliseconds 400

# open clipboard history, select newest, paste
[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 1300
[System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
Start-Sleep -Milliseconds 1800

$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save("C:\Windows\System32\_rayclip_tmp\paste.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "done"
