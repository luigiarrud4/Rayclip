"""Ambiente para subir o app com o DevTools (CDP) ligado **só quando o teste pede**.

O app lê `RAYCLIP_CDP` na abertura: `1` = porta 9222, ou o número da porta. Sem a
variável, nenhuma porta de depuração é aberta — o `tauri.conf.json` não carrega
mais `additionalBrowserArgs`, então o build de teste e o de release são o mesmo.

Uso:

    import cdp_env
    subprocess.Popen([EXE, "--open"], env=cdp_env.env_cdp(), ...)
"""

import os


def env_cdp(porta: int = 9222):
    """Ambiente do processo filho, com o CDP ligado na porta pedida."""
    return {**os.environ, "RAYCLIP_CDP": str(porta)}
