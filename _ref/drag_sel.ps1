Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class DS {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  public struct RECT { public int L,T,R,B; }
  public static void DragSlow(int x1,int y1,int x2,int y2){
    SetCursorPos(x1,y1); System.Threading.Thread.Sleep(250);
    mouse_event(0x0002,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(250);
    for(int i=1;i<=25;i++){ SetCursorPos(x1+(x2-x1)*i/25,y1+(y2-y1)*i/25); System.Threading.Thread.Sleep(25); }
    System.Threading.Thread.Sleep(150); mouse_event(0x0004,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(250);
  }
}
"@
$h=[DS]::FindWindow("Tauri Window","Rayclip")
$log="C:\Windows\System32\_rayclip_tmp\drag.txt"
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvD"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=800; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [DS]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("^a"); $k::SendWait("hello world selectme") }
    5 { $r=New-Object DS+RECT; [DS]::GetWindowRect($h,[ref]$r)|Out-Null
        $script:pos="$($r.L),$($r.T)" }
    # arrasta dentro do campo de texto (deve SELECIONAR, não mover a janela)
    6 { $r=New-Object DS+RECT; [DS]::GetWindowRect($h,[ref]$r)|Out-Null
        [DS]::DragSlow(($r.L+140),($r.T+26),($r.L+330),($r.T+26)) }
    7 { $r=New-Object DS+RECT; [DS]::GetWindowRect($h,[ref]$r)|Out-Null
        "selecao: janela em $($r.L),$($r.T) (antes $script:pos)" | Out-File $log -Encoding utf8
        [System.Windows.Forms.SendKeys]::SendWait("^c") }
    8 { $sel=[System.Windows.Forms.Clipboard]::GetText()
        "texto copiado pela selecao: '$sel'" | Out-File $log -Append -Encoding utf8 }
    # arrasta pela faixa do topo (deve MOVER a janela)
    9 { $r=New-Object DS+RECT; [DS]::GetWindowRect($h,[ref]$r)|Out-Null
        $script:p2="$($r.L),$($r.T)"
        [DS]::DragSlow(($r.L+720),($r.T+3),($r.L+830),($r.T+83)) }
    13 { $r=New-Object DS+RECT; [DS]::GetWindowRect($h,[ref]$r)|Out-Null
        "arrasto no topo: $($r.L),$($r.T) (antes $script:p2)" | Out-File $log -Append -Encoding utf8
        $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
