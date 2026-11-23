Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class S {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
}
"@

$n = Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $n) { Start-Process notepad.exe; Start-Sleep -Seconds 2; $n = Get-Process Notepad | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1 }
$h = $n.MainWindowHandle

$fg = [S]::GetForegroundWindow()
if ($fg -ne $h) {
    $p = 0
    $t = [S]::GetWindowThreadProcessId($fg, [ref]$p)
    [S]::AttachThreadInput([S]::GetCurrentThreadId(), $t, $true) | Out-Null
    [S]::SetForegroundWindow($h) | Out-Null
    [S]::AttachThreadInput([S]::GetCurrentThreadId(), $t, $false) | Out-Null
    Start-Sleep -Milliseconds 250
}
Write-Output ("foco no notepad: " + ([S]::GetForegroundWindow() -eq $h))

Set-Clipboard -Value "SMOKE-0-1-10-RELEASE"
Start-Sleep -Milliseconds 1200

[System.Windows.Forms.SendKeys]::SendWait("% ")
Start-Sleep -Milliseconds 900
Write-Output ("janela do rayclip na frente: " + ((Get-Process rayclip).MainWindowHandle -eq [S]::GetForegroundWindow()))
[System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
Start-Sleep -Milliseconds 800
Write-Output "enter enviado"
