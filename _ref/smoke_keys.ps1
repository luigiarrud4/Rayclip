param([string]$Action = "show")
# Teste de fumaca com teclado REAL (o CDP nao serve para esconder/mostrar, que
# passa pelo hook nativo). Usa keybd_event, sem SendKeys.
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Smoke {
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  public static void Key(byte vk, bool ctrl=false, bool shift=false, bool alt=false) {
    if (ctrl) keybd_event(0x11,0,0,IntPtr.Zero);
    if (shift) keybd_event(0x10,0,0,IntPtr.Zero);
    if (alt) keybd_event(0x12,0,0,IntPtr.Zero);
    System.Threading.Thread.Sleep(40);
    keybd_event(vk,0,0,IntPtr.Zero); System.Threading.Thread.Sleep(50);
    keybd_event(vk,0,2,IntPtr.Zero); System.Threading.Thread.Sleep(30);
    if (alt) keybd_event(0x12,0,2,IntPtr.Zero);
    if (shift) keybd_event(0x10,0,2,IntPtr.Zero);
    if (ctrl) keybd_event(0x11,0,2,IntPtr.Zero);
    System.Threading.Thread.Sleep(120);
  }
}
'@
$app = [Smoke]::FindWindow('Tauri Window','Rayclip')
function Visao { if ([Smoke]::IsWindowVisible($app)) { "visivel" } else { "escondida" } }
switch ($Action) {
  "show" {
    [Smoke]::Key(0x20, $false, $false, $true)   # Alt+Espaco
    Start-Sleep -Milliseconds 900
    "depois do Alt+Espaco: " + (Visao)
  }
  "keys" {
    # seta para baixo, Ctrl+K (acoes), Esc, Esc
    [Smoke]::Key(0x28)
    Start-Sleep -Milliseconds 250
    [Smoke]::Key(0x4B, $true)                   # Ctrl+K
    Start-Sleep -Milliseconds 400
    [Smoke]::Key(0x1B)                          # Esc fecha o painel
    Start-Sleep -Milliseconds 350
    "depois das teclas: " + (Visao)
    [Smoke]::Key(0x1B)                          # Esc esconde
    Start-Sleep -Milliseconds 450
    "depois do 2o Esc: " + (Visao)
  }
  "esc1" {
    [Smoke]::Key(0x1B)
    Start-Sleep -Milliseconds 500
    "depois de 1 Esc: " + (Visao)
  }
  default { "uso: show | keys | esc1" }
}
