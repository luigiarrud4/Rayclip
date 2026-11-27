@echo off
gh release view %1 --repo luigiarrud4/Rayclip --json assets | findstr /C:"\"name\":\"rayclip.exe\"" >nul 2>&1
if errorlevel 1 (echo FALHOU) else (echo OK-ASSET-PRESENTE)
