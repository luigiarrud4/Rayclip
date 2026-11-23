# Grava a mesma versão nos três lugares que precisam concordar:
#   package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml
# O Cargo.lock é atualizado depois pelo próprio cargo, na build.
#
# Uso:  powershell -File _ref/bump_version.ps1 0.1.24
param(
  [Parameter(Mandatory = $true)][string]$Version
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
  Write-Error "versao invalida: '$Version' (use X.Y.Z)"
  exit 1
}

# UTF-8 **sem BOM**: o serde do build do Tauri não aceita BOM no
# tauri.conf.json (e o npm tampouco gosta em package.json).
$utf8 = New-Object System.Text.UTF8Encoding($false)
function Escrever([string]$caminho, [string]$texto) {
  [System.IO.File]::WriteAllText($caminho, $texto, $utf8)
}

# --- package.json -----------------------------------------------------------
$pkgPath = Join-Path $root 'package.json'
$pkg = Get-Content -Raw -LiteralPath $pkgPath | ConvertFrom-Json
$pkg.version = $Version
Escrever $pkgPath (($pkg | ConvertTo-Json -Depth 20) + "`n")

# --- tauri.conf.json --------------------------------------------------------
$confPath = Join-Path $root 'src-tauri\tauri.conf.json'
$conf = Get-Content -Raw -LiteralPath $confPath | ConvertFrom-Json
$conf.version = $Version
Escrever $confPath (($conf | ConvertTo-Json -Depth 20) + "`n")

# --- Cargo.toml (só a primeira ocorrência de version = "…") -----------------
$cargoPath = Join-Path $root 'src-tauri\Cargo.toml'
$cargo = Get-Content -LiteralPath $cargoPath
$feito = $false
for ($i = 0; $i -lt $cargo.Count; $i++) {
  if (-not $feito -and $cargo[$i] -match '^version\s*=\s*"[^"]+"') {
    $cargo[$i] = 'version = "' + $Version + '"'
    $feito = $true
  }
}
if (-not $feito) { Write-Error 'nao achei version = "..." no Cargo.toml'; exit 1 }
Escrever $cargoPath (($cargo -join "`n") + "`n")

Write-Host "versao gravada: $Version"
exit 0
