# Fecha e reabre o Rayclip — rodar depois de TODO update (build/correção).
# Uso: powershell -NoProfile -File _ref/reiniciar.ps1 [-Release] [-Hidden]
#
# Por padrão abre com `--open` (janela na tela), que é o que os testes em _ref/
# precisam. `-Hidden` sobe só na bandeja, que é como o app nasce quando o
# Windows o inicia sozinho.
param([switch]$Release, [switch]$Hidden)

$exe = if ($Release) {
    "C:\Users\Lugia\Projects\rayclip\src-tauri\target\release\rayclip.exe"
} else {
    "C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
}

Get-Process rayclip -ErrorAction SilentlyContinue | Stop-Process -Force
for ($i = 0; $i -lt 20; $i++) {
    if (-not (Get-Process rayclip -ErrorAction SilentlyContinue)) { break }
    Start-Sleep -Milliseconds 150
}

if (-not (Test-Path $exe)) {
    Write-Output "binario nao encontrado: $exe (build primeiro)"
    exit 1
}

Remove-Item "$env:TEMP\rayclip.log" -ErrorAction SilentlyContinue
if ($Hidden) {
    Start-Process -FilePath $exe
} else {
    Start-Process -FilePath $exe -ArgumentList "--open"
}
Start-Sleep -Seconds 3

$p = Get-Process rayclip -ErrorAction SilentlyContinue | Select-Object -First 1
if ($p) {
    Write-Output ("reiniciado: pid {0} - janela '{1}' - log em $env:TEMP\rayclip.log" -f $p.Id, $p.MainWindowTitle)
} else {
    Write-Output "falhou: o processo nao subiu"
    exit 1
}
