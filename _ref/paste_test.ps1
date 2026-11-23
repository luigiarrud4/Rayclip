Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Fg {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
}
"@

$n = Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $n) { Write-Output "notepad nao achado"; exit 1 }

$h = $n.MainWindowHandle
[Fg]::ShowWindow($h, 9) | Out-Null          # SW_RESTORE
$fg = [Fg]::GetForegroundWindow()
if ($fg -ne $h) {
    $pid1 = 0; $pid2 = 0
    $t1 = [Fg]::GetWindowThreadProcessId($fg, [ref]$pid1)
    $t2 = [Fg]::GetWindowThreadProcessId($h, [ref]$pid2)
    # Anexa na thread do **primeiro plano** (é ela quem bloqueia o foco).
    [Fg]::AttachThreadInput([Fg]::GetCurrentThreadId(), $t1, $true) | Out-Null
    [Fg]::SetForegroundWindow($h) | Out-Null
    [Fg]::AttachThreadInput([Fg]::GetCurrentThreadId(), $t1, $false) | Out-Null
}
Start-Sleep -Milliseconds 350
$fg = [Fg]::GetForegroundWindow()
Write-Output ("notepad=0x{0:X} fg=0x{1:X} ok={2}" -f $h.ToInt64(), $fg.ToInt64(), ($fg -eq $h))

if ($fg -eq $h) {
    Start-Process 'C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe'
    Start-Sleep -Milliseconds 1300
    $lg = [Fg]::GetForegroundWindow()
    Write-Output ("depois de ativar o rayclip: fg=0x{0:X}" -f $lg.ToInt64())
}
