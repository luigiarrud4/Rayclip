Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices; using System.Text;
public class SEC {
  [DllImport("user32.dll")] public static extern bool OpenClipboard(IntPtr h);
  [DllImport("user32.dll")] public static extern bool EmptyClipboard();
  [DllImport("user32.dll")] public static extern bool CloseClipboard();
  [DllImport("user32.dll")] public static extern IntPtr SetClipboardData(uint f, IntPtr h);
  [DllImport("user32.dll")] public static extern uint RegisterClipboardFormat(string n);
  [DllImport("kernel32.dll")] public static extern IntPtr GlobalAlloc(uint f, UIntPtr n);
  [DllImport("kernel32.dll")] public static extern IntPtr GlobalLock(IntPtr h);
  [DllImport("kernel32.dll")] public static extern bool GlobalUnlock(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
  public static void SecureCopy(string text){
    OpenClipboard(IntPtr.Zero); EmptyClipboard();
    // CF_UNICODETEXT
    var bytes = Encoding.Unicode.GetBytes(text + "\0");
    IntPtr h = GlobalAlloc(0x0002, (UIntPtr)bytes.Length);
    IntPtr p = GlobalLock(h); Marshal.Copy(bytes,0,p,bytes.Length); GlobalUnlock(h);
    SetClipboardData(13, h);
    // marcador de "não registre isto"
    uint f = RegisterClipboardFormat("ExcludeClipboardContentFromMonitorProcessing");
    SetClipboardData(f, IntPtr.Zero);
    CloseClipboard();
  }
}
"@
Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\Lugia\AppData\Roaming\com.rayclip.app\clipboard\store.json" -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 600
Start-Process "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
Start-Sleep -Seconds 3
# 1) copia marcada como sensível (deve ser ignorada)
[SEC]::SecureCopy("SENHA-SUPER-SECRETA-123")
Start-Sleep -Seconds 1
# 2) copia normal (deve entrar)
Set-Clipboard -Value "TEXTO-NORMAL-OK"
Start-Sleep -Seconds 1
Start-Sleep -Milliseconds 300
