<#
Sobe o alvo_helper.py em integridade MEDIA usando o token do explorer.

Por que: o harness roda elevado (precisa fechar/reabrir o Rayclip de release) e
o Raycast, que e de integridade media, nao consegue digitar numa janela elevada
(UIPI). O caminho antigo era `schtasks /RL LIMITED`, que nesta maquina falha com
"ultimo resultado: 1"; aqui pegamos o token de um processo do usuario que ja esta
em integridade media (`explorer.exe`) e criamos o processo com ele.
#>

param(
    [string]$Exe = "",
    [string]$Arg = ""
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Tok {
  [DllImport("advapi32.dll", SetLastError = true)]
  public static extern bool OpenProcessToken(IntPtr p, uint access, out IntPtr tok);
  [DllImport("advapi32.dll", SetLastError = true)]
  public static extern bool DuplicateTokenEx(IntPtr tok, uint access, IntPtr attrs, int level, int type, out IntPtr novo);
  [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError = true)] public static extern bool CloseHandle(IntPtr h);
  [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  public static extern bool CreateProcessWithTokenW(IntPtr tok, uint flags, string app, string cmd, uint cflags,
      IntPtr env, string dir, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct STARTUPINFO { public int cb; public string r1, r2, r3; public int x, y, xs, ys, xc, yc, fill, flags; public short show, r4; public int r5, r6, r7; }
  [StructLayout(LayoutKind.Sequential)]
  public struct PROCESS_INFORMATION { public IntPtr hProc, hThread; public int pid, tid; }
}
"@

$explorer = Get-Process explorer | Select-Object -First 1
$h = [Tok]::OpenProcess(0x0400, $false, $explorer.Id)  # PROCESS_QUERY_INFORMATION
if ($h -eq [IntPtr]::Zero) { Write-Output "sem handle do explorer"; exit 1 }

$tok = [IntPtr]::Zero
if (-not [Tok]::OpenProcessToken($h, 0x0002 -bor 0x0008 -bor 0x0001, [ref]$tok)) {
    Write-Output "OpenProcessToken falhou: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"; exit 1
}
$dup = [IntPtr]::Zero
# TOKEN_DUPLICATE|TOKEN_ASSIGN_PRIMARY|TOKEN_QUERY = 0x0002|0x0001|0x0008
if (-not [Tok]::DuplicateTokenEx($tok, 0x000F01FF, [IntPtr]::Zero, 2, 1, [ref]$dup)) {
    Write-Output "DuplicateTokenEx falhou: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"; exit 1
}

$si = New-Object Tok+STARTUPINFO
$si.cb = [Runtime.InteropServices.Marshal]::SizeOf($si)
$pi = New-Object Tok+PROCESS_INFORMATION
$cmd = '"' + $Exe + '" "' + $Arg + '"'
$work = Split-Path -Parent $Arg
$ok = [Tok]::CreateProcessWithTokenW($dup, 0, $Exe, $cmd, 0x08000000, [IntPtr]::Zero, $work, [ref]$si, [ref]$pi)
if (-not $ok) {
    Write-Output "CreateProcessWithTokenW falhou: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    exit 1
}
Write-Output "alvo medio criado pid=$($pi.pid)"
