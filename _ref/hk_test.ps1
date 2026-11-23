Add-Type -AssemblyName System.Windows.Forms
$form = New-Object System.Windows.Forms.Form
$form.Width=400; $form.Height=200; $form.Text="hk test"
$t = New-Object System.Windows.Forms.Timer
$t.Interval = 1200
$t.Add_Tick({
  $t.Stop()
  [System.Windows.Forms.SendKeys]::SendWait("% ")
  Start-Sleep -Milliseconds 300
  Set-Content -Path "C:\Windows\System32\_rayclip_tmp\hk.txt" -Value "sent"
  Start-Sleep -Milliseconds 1200
  $form.Close()
})
$form.Add_Shown({ $t.Start() })
[System.Windows.Forms.Application]::Run($form)
