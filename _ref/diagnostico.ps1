# Diagnóstico da moldura do Rayclip — só lê, não muda nada.
#
# Rode com a janela do Rayclip **aberta e com a barra visível**, no PowerShell:
#   powershell -ExecutionPolicy Bypass -File diagnostico.ps1
#
# Ele imprime o estilo da janela, a área não-cliente (janela menos cliente), o
# estado do DWM e do tema. Com esses números dá para saber se a barra é a área
# não-cliente de verdade ou só uma pintura por cima.

Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;

public class Moldura {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
    public delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int idx);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out int v, int size);
    [DllImport("dwmapi.dll")] public static extern int DwmIsCompositionEnabled(out bool on);

    public static IntPtr achado = IntPtr.Zero;

    private static bool Procurar(IntPtr h, IntPtr p) {
        uint dono;
        GetWindowThreadProcessId(h, out dono);
        if (dono != pidAlvo) return true;
        var sb = new StringBuilder(256);
        GetClassName(h, sb, 256);
        string c = sb.ToString();
        if (c != "Tauri Window" && c != "Chrome_WidgetWin_1") return true;
        achado = h;
        return false;
    }

    public static uint pidAlvo = 0;

    public static IntPtr Achar(uint pid) {
        pidAlvo = pid;
        achado = IntPtr.Zero;
        EnumWindows(Procurar, IntPtr.Zero);
        return achado;
    }
}
"@

$procs = @(Get-Process rayclip -ErrorAction SilentlyContinue)
if ($procs.Count -eq 0) { Write-Host "Rayclip nao esta rodando." -ForegroundColor Red; exit 1 }

$proc = $null; $h = [IntPtr]::Zero
foreach ($cand in $procs) {
    $maybe = [Moldura]::Achar([uint32]$cand.Id)
    if ($maybe.ToInt64() -ne 0) { $proc = $cand; $h = $maybe; break }
}
if ($null -eq $proc -or $h.ToInt64() -eq 0) {
    Write-Host "Nao achei a janela do Rayclip. Abra a janela (atalho) e rode de novo." -ForegroundColor Red
    exit 1
}

$jan = New-Object Moldura+RECT
$cli = New-Object Moldura+RECT
[Moldura]::GetWindowRect($h, [ref]$jan) | Out-Null
[Moldura]::GetClientRect($h, [ref]$cli) | Out-Null

$estilo = [Moldura]::GetWindowLong($h, -16)
$ex     = [Moldura]::GetWindowLong($h, -20)
$nc = New-Object int
[Moldura]::DwmGetWindowAttribute($h, 1, [ref]$nc, 4) | Out-Null
$comp = $false
$hrComp = [Moldura]::DwmIsCompositionEnabled([ref]$comp)

$nomes = @{
  0x00C00000 = "WS_CAPTION"; 0x00040000 = "WS_THICKFRAME"; 0x00080000 = "WS_SYSMENU"
  0x10000000 = "WS_VISIBLE"; 0x02000000 = "WS_CLIPCHILDREN"; 0x04000000 = "WS_CLIPSIBLINGS"
  0x00020000 = "WS_MINIMIZEBOX"; 0x00010000 = "WS_MAXIMIZEBOX"
}
$tem = ($nomes.Keys | Sort-Object | Where-Object { $estilo -band $_ } | ForEach-Object { $nomes[$_] }) -join " "
$exNomes = @{ 0x00000008 = "TOPMOST"; 0x00000080 = "TOOLWINDOW"; 0x00080000 = "LAYERED" }
$temEx = ($exNomes.Keys | Sort-Object | Where-Object { $ex -band $_ } | ForEach-Object { $exNomes[$_] }) -join " "

$tema = (Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize" -ErrorAction SilentlyContinue)
$build = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion").CurrentBuild

Write-Host ""
Write-Host "Rayclip $($proc.VersionInfo.FileVersion)  pid $($proc.Id)  (PowerShell atual elevado: $(([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole('Administrators')))"
Write-Host "janela        : $($jan.Right - $jan.Left) x $($jan.Bottom - $jan.Top)  em ($($jan.Left),$($jan.Top))"
Write-Host "cliente       : $($cli.Right - $cli.Left) x $($cli.Bottom - $cli.Top)"
Write-Host ("faixa (NC)    : {0} px de altura, {1} px de largura" -f (($jan.Bottom - $jan.Top) - ($cli.Bottom - $cli.Top)), (($jan.Right - $jan.Left) - ($cli.Right - $cli.Left)))
Write-Host ("estilo        : 0x{0:X8}  {1}" -f $estilo, $tem)
Write-Host ("estilo ex     : 0x{0:X8}  {1}" -f $ex, $temEx)
Write-Host "DWM desenha NC: $([bool]$nc)   (True e o estado que pinta a barra)"
Write-Host ("composicao    : {0} (hr=0x{1:X8})" -f $comp, $hrComp)
Write-Host "Windows build : $build   tema claro do app: $($tema.AppsUseLightTheme)"
Write-Host ""
Write-Host "Se a faixa (NC) NAO for 0, a barra e a area nao-cliente de verdade."
Write-Host "Se for 0 e a barra aparece, ela esta sendo pintada por cima do conteudo."
