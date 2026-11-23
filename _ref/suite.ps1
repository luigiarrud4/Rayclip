Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
function Shot($n) {
  $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $x0=[int](($b.Width-750)/2); $y0=[int](($b.Height-475)*0.18)
  $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
  $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
  $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($x0-20),($y0-20),790,515)),$bmp.PixelFormat)
  $c.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose();$bmp.Dispose();$c.Dispose()
}
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driver"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=520; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1  { $k::SendWait("^+v") }                 # warm-up (may be swallowed)
    3  { $k::SendWait("^+v") }                 # open
    6  { $k::SendWait("128.45") }              # search number inside image
    9  { Shot "t_number" }
    10 { $k::SendWait("^a"); $k::SendWait("zzzq") }
    13 { Shot "t_empty" }
    14 { $k::SendWait("^a") ; $k::SendWait("{ESC}") }   # clear query, keep open? Esc closes
    15 { $k::SendWait("^+v") }                 # reopen
    18 { $k::SendWait("{DOWN}{DOWN}{DOWN}") }
    20 { $k::SendWait("^p") }                  # pin selected
    23 { Shot "t_pinned" }
    24 { $k::SendWait("^k") }
    26 { Shot "t_actions_entry" }
    27 { $k::SendWait("{ESC}") }
    29 { $k::SendWait("^+{BACKSPACE}") }       # clear all but pinned
    32 { Shot "t_cleared" }
    34 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form); Write-Output ok
