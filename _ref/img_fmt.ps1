Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class IE {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
}
"@
$h=[IE]::FindWindow("Tauri Window","Rayclip")
# garante uma entrada de imagem no topo
$img=[System.Drawing.Image]::FromFile("C:\Users\Lugia\Projects\rayclip\_ref\ocr-sample.png")
[System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()
Start-Sleep -Seconds 3
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driverI"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=650; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [IE]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("{HOME}") }
    5 { $k::SendWait("{ENTER}") }   # cola a primeira entrada (a imagem)
    8 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
Start-Sleep -Seconds 2
$d=[System.Windows.Forms.Clipboard]::GetDataObject()
Write-Output ("formats: " + ($d.GetFormats() -join ", "))
Write-Output ("has image: " + [System.Windows.Forms.Clipboard]::ContainsImage())
if ([System.Windows.Forms.Clipboard]::ContainsImage()) {
  $i=[System.Windows.Forms.Clipboard]::GetImage()
  Write-Output ("size: " + $i.Width + "x" + $i.Height)
  $i.Dispose()
}
