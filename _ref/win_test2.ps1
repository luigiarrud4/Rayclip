Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class WD {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
  public static void Drag(int x1,int y1,int x2,int y2){
    SetCursorPos(x1,y1); System.Threading.Thread.Sleep(200);
    mouse_event(0x0002,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(200);
    for(int i=1;i<=20;i++){ SetCursorPos(x1+(x2-x1)*i/20, y1+(y2-y1)*i/20); System.Threading.Thread.Sleep(30); }
    System.Threading.Thread.Sleep(180); mouse_event(0x0004,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(250);
  }
}
"@
$log = "C:\Windows\System32\_rayclip_tmp\win_test.txt"
"start" | Out-File $log -Encoding utf8
$h=[WD]::FindWindow("Tauri Window","Rayclip")
"hwnd=$h visible=" + [WD]::IsWindowVisible($h) | Out-File $log -Append -Encoding utf8
$r=New-Object WD+RECT; [WD]::GetWindowRect($h,[ref]$r)|Out-Null
"before: $($r.L),$($r.T) $($r.R-$r.L)x$($r.B-$r.T)" | Out-File $log -Append -Encoding utf8
[WD]::Drag(($r.L+300),($r.T+18),($r.L+440),($r.T+108))
$r2=New-Object WD+RECT; [WD]::GetWindowRect($h,[ref]$r2)|Out-Null
"after drag: $($r2.L),$($r2.T) $($r2.R-$r2.L)x$($r2.B-$r2.T)" | Out-File $log -Append -Encoding utf8
[WD]::Drag(($r2.R-2),($r2.B-2),($r2.R+168),($r2.B+126))
$r3=New-Object WD+RECT; [WD]::GetWindowRect($h,[ref]$r3)|Out-Null
"after resize: $($r3.L),$($r3.T) $($r3.R-$r3.L)x$($r3.B-$r3.T)" | Out-File $log -Append -Encoding utf8
