# Imprime a versão atual do projeto (a de src-tauri/tauri.conf.json, que é a
# que vai para o exe). Usado pelo deploy.bat, sem pipes no cmd.
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$conf = Get-Content -Raw -LiteralPath (Join-Path $root 'src-tauri\tauri.conf.json') | ConvertFrom-Json
Write-Output $conf.version
