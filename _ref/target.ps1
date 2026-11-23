Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$form = New-Object System.Windows.Forms.Form
$form.Text = "Rayclip Paste Target"
$form.Width = 620; $form.Height = 320
$tb = New-Object System.Windows.Forms.TextBox
$tb.Multiline = $true; $tb.Dock = "Fill"; $tb.Font = New-Object System.Drawing.Font("Consolas", 12)
$form.Controls.Add($tb)

$state = 0
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 1000
$timer.Add_Tick({
  $script:state++
  switch ($script:state) {
    1 { [System.Windows.Forms.Clipboard]::SetText("HELLO-RAYCLIP-PASTE-TEST-123") }
    3 { [System.Windows.Forms.SendKeys]::SendWait("% ") }
    5 { [System.Windows.Forms.SendKeys]::SendWait("{ENTER}") }
    8 {
      Set-Content -Path "C:\Windows\System32\_rayclip_tmp\out.txt" -Value $tb.Text
      $timer.Stop(); $form.Close()
    }
  }
})
$form.Add_Shown({ $timer.Start(); $tb.Focus() })
[System.Windows.Forms.Application]::Run($form)
