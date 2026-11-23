Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices; using System.Text;
public class G {
  [DllImport("user32.dll")] public static extern bool OpenClipboard(IntPtr h);
  [DllImport("user32.dll")] public static extern bool EmptyClipboard();
  [DllImport("user32.dll")] public static extern bool CloseClipboard();
  [DllImport("user32.dll")] public static extern IntPtr SetClipboardData(uint f, IntPtr h);
  [DllImport("user32.dll")] public static extern uint RegisterClipboardFormat(string n);
  [DllImport("kernel32.dll")] public static extern IntPtr GlobalAlloc(uint f, UIntPtr n);
  [DllImport("kernel32.dll")] public static extern IntPtr GlobalLock(IntPtr h);
  [DllImport("kernel32.dll")] public static extern bool GlobalUnlock(IntPtr h);
  public static void PutGif(string path){
    byte[] bytes = System.IO.File.ReadAllBytes(path);
    OpenClipboard(IntPtr.Zero); EmptyClipboard();
    IntPtr h = GlobalAlloc(0x0002,(UIntPtr)bytes.Length);
    IntPtr p = GlobalLock(h); Marshal.Copy(bytes,0,p,bytes.Length); GlobalUnlock(h);
    SetClipboardData(RegisterClipboardFormat("GIF"), h);
    CloseClipboard();
  }
}
"@
[G]::PutGif("C:\Users\Lugia\Projects\rayclip\_ref\anim.gif")
Write-Output "gif no clipboard"
