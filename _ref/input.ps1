Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Inp {
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static void Click(int x, int y) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(120);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero);
  }
  public static void AltSpace() {
    keybd_event(0x12, 0, 0, IntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 0, IntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 0x0002, IntPtr.Zero);
    keybd_event(0x12, 0, 0x0002, IntPtr.Zero);
  }
  public static string Fg() {
    IntPtr h = GetForegroundWindow();
    RECT r; GetWindowRect(h, out r);
    return h + " [" + r.L + "," + r.T + "," + r.R + "," + r.B + "]";
  }
}
'@
$arg = $args[0]
switch ($arg) {
  "altspace" { [Inp]::AltSpace(); "alt+space enviado" }
  "fg" { [Inp]::Fg() }
  "click" { [Inp]::Click([int]$args[1], [int]$args[2]); "click $($args[1]),$($args[2])" }
  default { "uso: altspace | fg | click X Y" }
}
