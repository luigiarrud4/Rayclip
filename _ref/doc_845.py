import io, subprocess

DOC = "C:/Users/Lugia/Projects/rayclip/DOCUMENTACAO.md"
SECAO = """### 8.45 Auto-updater verificado ponta a ponta (0.0.9 -> 0.1.33)

A pergunta era "o auto-updater está funcionando?". Estava, mas nunca tinha sido
testado com uma atualização de verdade — só com testes de unidade das peças.

**Como testar sem estragar a instalação:** um build de debug declarando
**0.0.9** (bump temporário em `Cargo.toml`) + CDP ligado, rodando de
`src-tauri/target/debug/`. O swap acontece na pasta do próprio exe, então a
instalação em `AppData\\Local\\Rayclip` nem foi tocada.

```
invoke('update_check', {force: true})
invoke('update_install')
```

**Resultados:**

| Etapa | O que saiu |
|---|---|
| check | `{current: "0.0.9", version: "0.1.33", tag: "v0.1.33", size: 10871296, hasAsset: true}` |
| install | retornou `"0.1.33"`; log `update: v0.1.33 disponivel` |
| exe antes | 25.950.720 B (debug), sha `cf08df98e6f01bb5…` |
| exe depois | **10.871.296 B**, sha `3a4ad814c5e7550c…` — **igual ao asset publicado** |
| backup | `rayclip.exe.old` = o exe de debug, exatamente como o desenho prevê |
| processo novo | versão **0.1.33** rodando do mesmo caminho |
| abertura seguinte | `rayclip.exe.old` **apagado** (a regra "sobrevive à primeira execução") |

O sha256 confere porque o updater é *fail-closed*: sem checksum ou com hash
diferente ele não instala (testes `download_sem_checksum_e_recusado`,
`download_de_outro_host_e_recusado`, `versao_maior_e_mais_nova`).

**Depois do teste:** `Cargo.toml` e `tauri.conf.json` restaurados (`git diff`
vazio, `grep -c remote-debugging` = 0) e o build de debug refeito com log.

**Observação:** o caminho "já estou na última versão" não baixa nada — é o
estado normal da máquina (0.1.33 instalada = 0.1.33 publicada).

"""

ALVO = "### 8.44 Busca e filtro no Rust"
s = io.open(DOC, encoding="utf-8").read()
assert ALVO in s
s = s.replace(ALVO, SECAO + ALVO, 1)
io.open(DOC, "w", encoding="utf-8", newline="").write(s)
print("doc 8.45 ok")

# app instalado volta para a bandeja
subprocess.run(["powershell", "-NoProfile", "-Command",
                "Start-Process 'C:\\Users\\Lugia\\AppData\\Local\\Rayclip\\rayclip.exe'"],
               check=False)
