Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 900
1..3 | ForEach-Object { [System.Windows.Forms.SendKeys]::SendWait("{UP}"); Start-Sleep -Milliseconds 90 }
Write-Output "aberto, selecao movida para cima"
