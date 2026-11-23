"""Captura a maior janela visivel de um processo (para print do Raycast, que nao
expoe MainWindowHandle por ser WPF com janela customizada)."""
param(
    [Parameter(Mandatory = $true)][string]$Processo,
    [Parameter(Mandatory = $true)][string]$Saida
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class E {
  public delegate bool Cb(IntPtr h, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool EnumWindows(Cb cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
}
"@

$alvo = (Get-Process -Name $Processo -ErrorAction Stop).Id
$melhor = @{ Area = 0; Handle = [IntPtr]::Zero }

$cb = [E+Cb]{
    param($h, $l)
    $pid2 = 0
    [void][E]::GetWindowThreadProcessId($h, [ref]$pid2)
    if ($pid2 -eq $alvo -and [E]::IsWindowVisible($h)) {
        $r = New-Object E+RECT
        [void][E]::GetWindowRect($h, [ref]$r)
        $area = ($r.R - $r.L) * ($r.B - $r.T)
        if ($area -gt $script:melhor.Area) {
            $script:melhor.Area = $area
            $script:melhor.Handle = $h
        }
    }
    return $true
}

[void][E]::EnumWindows($cb, [IntPtr]::Zero)
if ($melhor.Area -le 0) { Write-Output "nenhuma janela visivel"; exit 1 }

$h = $melhor.Handle
$r = New-Object E+RECT
[void][E]::GetWindowRect($h, [ref]$r)
$w = $r.R - $r.L
$hh = $r.B - $r.T

$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $hh))
$bmp.Save($Saida, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $w x $hh -> $Saida"
