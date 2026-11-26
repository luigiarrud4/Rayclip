; Ganchos do instalador NSIS do Rayclip.
;
; 1. **PREINSTALL**: fecha o Rayclip que estiver rodando. Sem isso o arquivo
;    `rayclip.exe` esta em uso e a troca falha. Como o app roda elevado, quem
;    precisa de elevacao e o instalador — por isso o `installMode` e `perMachine`
;    no `tauri.conf.json` (UAC na entrada).
; 2. **POSTINSTALL**: liga "iniciar com o Windows" usando o **proprio app**
;    (`--iniciar-com-windows`), que cria a tarefa agendada elevada do jeito que o
;    resto do codigo espera. Falha aqui nao derruba a instalacao: se der errado, o
;    usuario ainda pode ligar em Configuracoes > Sistema.
;
; Os filhos do WebView2 morrem junto com o processo pai; o que sobrar e limpo
; pelo proprio Rayclip no proximo arranque (`update::limpar_inicio`).
;
; Atencao: este arquivo e NSIS, nao shell. Caminho com barra invertida sofre com
; escape em qualquer ferramenta que reescreva o arquivo — edite a mao.

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Fechando o Rayclip em execucao..."
  ; /IM fecha todas as instancias; o /F e necessario porque o app roda elevado.
  nsExec::ExecToLog 'taskkill /IM rayclip.exe /F'
  Pop $0
  ; Um respiro para o Windows liberar o arquivo antes de copiar por cima.
  Sleep 900

  ; Migracao: ate a 0.1.59 a instalacao era por usuario (em LOCALAPPDATA).
  ; Se ela existe, sai daqui — senao sobram duas copias, um atalho apontando
  ; para um exe antigo e o desinstalador da versao antiga. Os DADOS nao ficam
  ; ai: historico e ajustes vivem em APPDATA e nao sao tocados.
  IfFileExists "$LOCALAPPDATA\Rayclip\rayclip.exe" 0 +5
    DetailPrint "Removendo a instalacao antiga por usuario..."
    RMDir /r "$LOCALAPPDATA\Rayclip"
    Delete "$APPDATA\Microsoft\Windows\Start Menu\Programs\Rayclip.lnk"
    Delete "$DESKTOP\Rayclip.lnk"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Ligando o Rayclip junto com o Windows..."
  ExecWait '"$INSTDIR\rayclip.exe" --iniciar-com-windows'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Desinstalar tambem tira o app do logon: senao sobra uma tarefa agendada
  ; apontando para um executavel que nao existe mais.
  ExecWait '"$INSTDIR\rayclip.exe" --remover-do-inicio'
  nsExec::ExecToLog 'schtasks /Delete /TN Rayclip /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM rayclip.exe /F'
  Pop $0
  Sleep 700
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
