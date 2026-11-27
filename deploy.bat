@echo off
chcp 65001 >nul 2>&1
setlocal EnableDelayedExpansion
cd /d "%~dp0"

:: ===========================================================================
::  RAYCLIP - DEPLOY
::
::  Gera a versao, compila, calcula o SHA-256 e publica a release no GitHub.
::  O proprio app le essas releases para se atualizar (Settings > Updates),
::  entao o asset "rayclip.exe" e obrigatorio: e ele que o updater baixa.
::
::  Uso:
::    deploy.bat               -> sobe o patch (0.1.24 -> 0.1.25) e publica
::    deploy.bat 0.2.0         -> publica essa versao
::    deploy.bat --build-only  -> compila e calcula o hash, sem publicar
::
::  Antes: o repositorio precisa existir e o gh estar logado.
::    gh auth login
::    gh repo create luigiarrud4/Rayclip --public --source . --remote origin --push
:: ===========================================================================

set "REPO=luigiarrud4/Rayclip"
set "TARGET_BRANCH=main"
set "EXE=src-tauri\target\release\rayclip.exe"
set "HASHFILE=src-tauri\target\release\rayclip.exe.sha256"

if exist "C:\Program Files\Git\cmd\git.exe" set "PATH=C:\Program Files\Git\cmd;C:\Program Files\Git\bin;%PATH%"
if exist "C:\Program Files\GitHub CLI\gh.exe" set "PATH=C:\Program Files\GitHub CLI;%PATH%"

echo.
echo  ===============================================
echo        RAYCLIP - DEPLOY
echo  ===============================================
echo.

:: --- argumentos ------------------------------------------------------------
set "BUILD_ONLY="
set "NEXT="
for %%A in (%*) do (
    if /i "%%~A"=="--build-only" (set "BUILD_ONLY=1") else (set "NEXT=%%~A")
)

:: --- versao atual ----------------------------------------------------------
for /f "usebackq delims=" %%V in (`powershell -NoProfile -ExecutionPolicy Bypass -File "_ref\get_version.ps1"`) do set "CURRENT=%%V"
if not defined CURRENT (
    echo  ERRO: nao consegui ler a versao em src-tauri\tauri.conf.json
    goto :fim_erro
)

if not defined NEXT (
    for /f "tokens=1,2,3 delims=." %%A in ("!CURRENT!") do (
        set /a "PATCH=%%C+1"
        set "NEXT=%%A.%%B.!PATCH!"
    )
)

echo   Versao atual : v!CURRENT!
echo   Versao nova  : v!NEXT!
echo.

if defined BUILD_ONLY goto :compile

:: --- checagens de publicacao ----------------------------------------------
gh auth status -h github.com >nul 2>&1
if errorlevel 1 (
    echo  ERRO: gh nao esta logado. Rode: gh auth login -h github.com -w
    goto :fim_erro
)
gh repo view %REPO% >nul 2>&1
if errorlevel 1 (
    echo  ERRO: repositorio %REPO% nao existe.
    echo        Crie uma vez com:
    echo          gh repo create %REPO% --public --source . --remote origin --push
    goto :fim_erro
)

:: --- versao nos tres arquivos ---------------------------------------------
echo  [1/5] Gravando a versao...
powershell -NoProfile -ExecutionPolicy Bypass -File "_ref\bump_version.ps1" !NEXT!
if errorlevel 1 goto :fim_erro

:: --- build ----------------------------------------------------------------
:compile
echo.
echo  [2/5] Compilando (release + instalador)...
powershell -NoProfile -Command "Get-Process rayclip -ErrorAction SilentlyContinue | Stop-Process -Force"
call npx tauri build
if errorlevel 1 (
    echo  ERRO: a build falhou. Nada foi publicado.
    goto :fim_erro
)
if not exist "!EXE!" (
    echo  ERRO: nao achei !EXE!
    goto :fim_erro
)

:: --- hash -----------------------------------------------------------------
echo.
echo  [3/5] Calculando o SHA-256...
for /f "skip=1 tokens=1" %%H in ('certutil -hashfile "!EXE!" SHA256') do (
    if not defined HASH set "HASH=%%H"
)
if not defined HASH (
    echo  ERRO: certutil nao devolveu o hash.
    goto :fim_erro
)
> "!HASHFILE!" echo !HASH!  rayclip.exe
echo   SHA-256: !HASH!

if defined BUILD_ONLY (
    echo.
    echo  Build pronto: !EXE!
    echo  Nada foi publicado ^(build-only^).
    goto :fim_ok
)

:: --- commit + push --------------------------------------------------------
echo.
echo  [4/5] Commit e push...
git add -A
if errorlevel 1 (
    echo  ERRO: git add falhou. Nada foi publicado.
    goto :fim_erro
)
git commit -m "v!NEXT!" >nul 2>&1
if errorlevel 1 (
    echo   (nada novo para commitar)
)
git push origin !TARGET_BRANCH!
if errorlevel 1 (
    echo  ERRO: git push falhou. Nada foi publicado.
    goto :fim_erro
)

:: --- release --------------------------------------------------------------
echo.
echo  [5/5] Publicando a release v!NEXT!...
set "SETUP=src-tauri\target\release\bundle\nsis\Rayclip_!NEXT!_x64-setup.exe"

gh release view "v!NEXT!" --repo %REPO% >nul 2>&1
if not errorlevel 1 (
    echo   Release v!NEXT! ja existe: substituindo os arquivos...
    gh release upload "v!NEXT!" --repo %REPO% "!EXE!" "!HASHFILE!" "!SETUP!" --clobber
    if errorlevel 1 goto :release_falhou
    goto :release_ok
)

gh release create "v!NEXT!" --repo %REPO% --target !TARGET_BRANCH! --title "Rayclip v!NEXT!" --generate-notes "!EXE!" "!HASHFILE!" "!SETUP!"
if errorlevel 1 goto :release_falhou

:release_ok
:: Checagem obrigatoria: o updater do app baixa o asset "rayclip.exe".
:: (A v0.1.61-0.1.64 foram publicadas na mao sem ele e o botao de atualizar
:: ficou morto ate o backfill - nunca de novo sem esse asset.)
gh release view "v!NEXT!" --repo %REPO% --json assets | findstr /C:"\"name\":\"rayclip.exe\"" >nul 2>&1
if errorlevel 1 (
    echo  ERRO: a release v!NEXT! ficou SEM o asset rayclip.exe.
    echo        O updater do app depende dele. Publique com:
    echo          gh release upload "v!NEXT!" "!EXE!" "!HASHFILE!" --clobber
    goto :fim_erro
)

echo.
echo  ===============================================
echo    RELEASE v!NEXT! PUBLICADA
echo  ===============================================
echo    https://github.com/%REPO%/releases
echo    O app se atualiza por Settings ^> Updates
echo    (ou pelo aviso no rodape quando houver versao nova).
echo.
echo    Reiniciando o Rayclip na versao nova...
powershell -NoProfile -ExecutionPolicy Bypass -File "_ref\reiniciar.ps1" -Release >nul 2>&1
goto :fim_ok

:release_falhou
echo  ERRO ao publicar a release.
goto :fim_erro

:fim_erro
echo.
exit /b 1

:fim_ok
echo.
exit /b 0
