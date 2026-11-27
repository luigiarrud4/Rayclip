#![windows_subsystem = "windows"]

mod capture;
mod clipimg;
mod import;
mod key;
mod linkmeta;
mod ocr;
mod search;
mod settings;
mod sqlite;
mod store;
mod sysinteg;
mod thumbs;
mod update;
mod webmeta;
mod win32;

use clipboard_win::Setter;
use serde::Deserialize;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use store::{Entry, Store};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub store: Mutex<Store>,
    pub settings: Mutex<settings::Settings>,
    pub last_foreground: AtomicIsize,
    /// Últimas sequências de clipboard que NÓS geramos (não recapturar).
    /// É uma lista: o paste_many faz várias cópias seguidas e o watcher pode
    /// ver qualquer uma delas — um valor único ignorava as intermediárias.
    pub ignore_sequences: Mutex<Vec<u32>>,
    pub shown_at: Mutex<std::time::Instant>,
    /// Trava o auto-hide enquanto um diálogo nativo/overlay está aberto.
    pub dialog_open: std::sync::atomic::AtomicBool,
}

/// Marca `dialog_open` e solta ao sair do escopo (mesmo com erro).
struct DialogGuard(AppHandle);
impl Drop for DialogGuard {
    fn drop(&mut self) {
        if let Some(state) = self.0.try_state::<AppState>() {
            state.dialog_open.store(false, Ordering::SeqCst);
        }
    }
}

/// Últimos atalhos que **realmente** ficaram registrados (abrir, colar).
///
/// Serve de rede de segurança: se o usuário escolher um atalho que outro
/// programa já usa, o app volta para o que funcionava em vez de ficar sem
/// atalho nenhum até reiniciar.
static ATALHOS_ATIVOS: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

/// (Re)registra os atalhos globais a partir das configurações.
///
/// Cada atalho é registrado **sozinho**: com o Raycast aberto (que usa
/// `Ctrl+Shift+V`) um registro em bloco falhava e derrubava também o
/// `Alt+Space`, deixando o app sem atalho nenhum e sem aviso.
///
/// Quando o registro falha, `cfg` é **revertido** para o atalho que estava
/// funcionando e a mensagem devolvida é um código estável
/// (`hotkey-conflict|show|<tentado>|<mantido>`) — a interface traduz. O texto
/// cru do erro (`HotKey { mods: Modifiers(ALT), … }`) nunca chega ao usuário.
fn apply_shortcuts(app: &AppHandle, cfg: &mut settings::Settings) -> Vec<String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut warnings = Vec::new();
    let anteriores = ATALHOS_ATIVOS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let mut feitos: Vec<String> = Vec::new();
    let mut bons: Vec<Option<String>> = vec![None, None];

    for (indice, combo) in [cfg.show_hotkey.clone(), cfg.paste_hotkey.clone()]
        .into_iter()
        .enumerate()
    {
        let combo = combo.trim().to_string();
        if combo.is_empty() || feitos.contains(&combo) {
            continue;
        }
        feitos.push(combo.clone());
        let handle = app.clone();
        let res = gs.on_shortcut(combo.as_str(), move |_a, _s, event| {
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                toggle_window(&handle);
            }
        });
        match res {
            Ok(()) => {
                log(&format!("atalho registrado: {combo}"));
                bons[indice] = Some(combo);
            }
            Err(e) => {
                log(&format!("atalho {combo} indisponivel: {e}"));
                // Volta para o último atalho bom deste campo, se houver.
                let antigo = anteriores.as_ref().map(|(a, b)| {
                    if indice == 0 {
                        a.clone()
                    } else {
                        b.clone()
                    }
                });
                let mut mantido = None;
                if let Some(antigo) = antigo.filter(|a| !a.trim().is_empty() && *a != combo) {
                    let h2 = app.clone();
                    let ok = gs
                        .on_shortcut(antigo.as_str(), move |_a, _s, event| {
                            if event.state()
                                == tauri_plugin_global_shortcut::ShortcutState::Pressed
                            {
                                toggle_window(&h2);
                            }
                        })
                        .is_ok();
                    if ok {
                        log(&format!("atalho {combo} recusado; mantido {antigo}"));
                        if indice == 0 {
                            cfg.show_hotkey = antigo.clone();
                        } else {
                            cfg.paste_hotkey = antigo.clone();
                        }
                        bons[indice] = Some(antigo.clone());
                        mantido = Some(antigo);
                    }
                }
                let campo = if indice == 0 { "show" } else { "paste" };
                match mantido {
                    Some(k) => warnings.push(format!("hotkey-conflict|{campo}|{combo}|{k}")),
                    None => warnings.push(format!("hotkey-failed|{campo}|{combo}")),
                }
            }
        }
    }

    // Memória dos atalhos que ficaram valendo (mesmo que só um tenha entrado).
    let memoria = (
        bons[0]
            .clone()
            .or_else(|| anteriores.as_ref().map(|(a, _)| a.clone()))
            .unwrap_or_default(),
        bons[1]
            .clone()
            .or_else(|| anteriores.as_ref().map(|(_, b)| b.clone()))
            .unwrap_or_default(),
    );
    *ATALHOS_ATIVOS.lock().unwrap_or_else(|e| e.into_inner()) = Some(memoria);
    warnings
}

/// Aplica o que depende das configurações — **só o que mudou**.
///
/// Antes isto rodava inteiro em toda gravação: registrar atalhos globais outra
/// vez, `schtasks` (abrir a tarefa do autostart) e `reg` (ler e escrever o
/// `DisabledHotkeys`). Cada um desses abre um **processo**, e a conta medida era
/// de **~1,1 s por gravação** — com o slider de transparência disparando a cada
/// passo do mouse, a janela passava a vida esperando e parecia travada.
///
/// Com o diff, mexer em cor/opacidade/idioma custa ~0 (nada disto depende
/// desses campos) e só trocar atalho/autostart/Win+V paga o preço que tem de
/// pagar.
fn apply_settings(
    app: &AppHandle,
    antes: Option<&settings::Settings>,
    cfg: &mut settings::Settings,
) -> Vec<String> {
    // Sem `antes` (início do app) tudo é aplicado; com `antes`, só o que mudou.
    macro_rules! mudou {
        ($cmp:expr) => {
            match antes {
                Some(a) => $cmp(a, cfg),
                None => true,
            }
        };
    }
    let mut warnings = Vec::new();
    if mudou!(|a: &settings::Settings, b: &settings::Settings| a.show_hotkey != b.show_hotkey
        || a.paste_hotkey != b.paste_hotkey)
    {
        warnings.extend(apply_shortcuts(app, cfg));
    }
    if mudou!(|a: &settings::Settings, b: &settings::Settings| a.max_entries != b.max_entries
        || a.retention_days != b.retention_days)
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state.store);
        store.set_max_entries(cfg.max_entries);
        store.purge_older_than(cfg.retention_days);
    }
    if mudou!(|a: &settings::Settings, b: &settings::Settings| a.replace_win_v != b.replace_win_v) {
        if let Err(e) = sysinteg::set_replace_win_v(cfg.replace_win_v) {
            warnings.push(format!("win+v: {e}"));
        }
    }
    if mudou!(|a: &settings::Settings, b: &settings::Settings| a.launch_at_login != b.launch_at_login) {
        if let Err(e) = sysinteg::set_startup(cfg.launch_at_login) {
            warnings.push(format!("startup: {e}"));
        }
    }
    if mudou!(|a: &settings::Settings, b: &settings::Settings| a.translucent != b.translucent) {
        if let Some(win) = app.get_webview_window("main") {
            if let Ok(h) = win.hwnd() {
                win32::apply_backdrop(
                    h.0 as isize,
                    if cfg.translucent && sysinteg::windows_transparency() {
                        2
                    } else {
                        1
                    },
                    cfg.theme != "light",
                );
            }
        }
    }
    warnings
}

/// Arrasto para fora: payload pronto para o `dataTransfer` do Chromium.
///
/// O Explorer só aceita `CF_HDROP` (arquivo de verdade), então o arrasto é feito
/// pelo próprio Chromium com a receita `DownloadURL`: mime, nome e uma URL de
/// objeto. Ver `src/components/EntryList.tsx`.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DragPayload {
    name: String,
    mime: String,
    /// Bytes em base64 — viram um `blob:` no frontend (o arquivo que o Explorer
    /// cria). Vazio quando o conteúdo é grande demais.
    data: String,
    /// Texto puro para soltar em editor/chat (Notepad, Discord, Word). `None`
    /// para imagens/GIFs.
    text: Option<String>,
    /// O texto é um link (o frontend também anuncia `text/uri-list`).
    link: bool,
}

/// Tamanho máximo que vira arrasto. Acima disso o arquivo não é lido: copiar
/// centenas de MB para base64 travava a janela ("não respondendo").
const MAX_ARRASTO: u64 = 128 * 1024 * 1024;
/// Texto oferecido como `text/plain` no arrasto.
const MAX_ARRASTO_TEXTO: usize = 200_000;

/// Conteúdo + nome para arrastar a entrada para uma pasta/outro app.
///
/// base64 padrão. A crate 0.22 não expõe mais `base64::encode`/`decode` no topo.
fn b64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn b64_decode(texto: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(texto).ok()
}

/// Caminho que o arrasto do **sistema** pode entregar: local (`C:\...`) e que
/// ainda existe no disco.
///
/// Caminho de rede (`\\servidor\...`) derruba o plugin de arrasto (bug aberto do
/// drag-rs #72): recusar é melhor do que levar o app junto.
fn caminho_de_arrasto(p: &std::path::Path) -> Result<String, String> {
    let s = p.to_string_lossy().to_string();
    if s.starts_with("\\\\") {
        return Err("network path: Chromium drag instead".into());
    }
    if !p.is_file() {
        return Err("the file is gone from disk".into());
    }
    Ok(s)
}

/// Caminhos locais que o arrasto do **sistema** entrega (`CF_HDROP`, via OLE).
///
/// O `DownloadURL` do Chromium leva os **bytes** e por isso tem teto (medido:
/// o `.zip` de 3 MB vira arquivo, o de 25 MB não vira nada); o OLE leva o
/// **caminho** — não copia nada e não tem limite de tamanho. Arquivo que já está
/// no disco sai com o caminho original; imagem que só existe no banco cifrado é
/// materializada uma vez em `<dados>/drag/<id>.<ext>`.
///
/// `Err` quando não há arquivo (texto, link): aí o frontend continua com o
/// caminho do Chromium, que funciona bem para texto.
#[tauri::command]
async fn drag_files(state: State<'_, AppState>, id: String) -> Result<Vec<String>, String> {
    let (entry, imagem, dir) = {
        let store = lock_store(&state.store);
        let Some(e) = store.get(&id).cloned() else {
            return Err("entry not found".into());
        };
        let img = if e.has_image {
            store.image_bytes(&id)
        } else {
            None
        };
        (e, img, store.dir.clone())
    };
    // Fora da thread de UI: materializar imagem do banco é encode de verdade.
    tauri::async_runtime::spawn_blocking(move || {
        arquivos_da_entrada(&entry, imagem, &dir, &id).map(|ps| {
            ps.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Roda numa thread de bloqueio: o hover da lista chama isto para adiantar o
/// payload, e ler um `.zip` grande no meio do laço de eventos congelava a janela.
#[tauri::command]
async fn drag_payload(state: State<'_, AppState>, id: String) -> Result<DragPayload, String> {
    let (entry, imagem) = {
        let store = lock_store(&state.store);
        let Some(e) = store.get(&id).cloned() else {
            return Err("entry not found".into());
        };
        let img = if e.has_image {
            store.image_bytes(&id)
        } else {
            None
        };
        (e, img)
    };
    tauri::async_runtime::spawn_blocking(move || monta_payload(&entry, imagem, &id))
        .await
        .map_err(|e| e.to_string())?
}

/// Monta o payload de arrasto (roda fora da thread de UI).
fn monta_payload(
    entry: &crate::store::Entry,
    imagem: Option<Vec<u8>>,
    id: &str,
) -> Result<DragPayload, String> {
    let mut ext = normalize_ext("", entry);
    let mut stem = drag_file_stem(entry);
    let mut mime = String::from("text/plain");
    let mut texto: Option<String> = None;

    let bytes: Vec<u8> = if let Some(img) = imagem {
        let is_gif = entry.gif || img.starts_with(b"GIF8");
        mime = if is_gif {
            "image/gif".into()
        } else {
            match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "webp" => "image/webp",
                "bmp" => "image/bmp",
                _ => "image/png",
            }
            .to_string()
        };
        // Sem conversão quando o formato guardado já é o pedido: o arrasto tem
        // que sair na hora do `dragstart`, e reencodar levava ~170 ms.
        if (is_gif && ext == "gif") || (!is_gif && ext == "png") {
            img
        } else {
            let tmp = std::env::temp_dir().join(format!("rayclip-drag-{id}.{ext}"));
            encode_image(&img, is_gif, &tmp.to_string_lossy(), &ext)?;
            let lido = std::fs::read(&tmp).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&tmp);
            lido
        }
    } else if let Some(p) = entry
        .paths
        .first()
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_file())
    {
        // O arrasto de um arquivo do disco leva a extensão e o nome **do
        // arquivo** (um `.zip` continuava `.txt`, um `.png` virava `.txt`).
        if let Some(e) = p.extension().map(|e| e.to_string_lossy().to_lowercase()) {
            if !e.is_empty() {
                ext = e;
            }
        }
        if let Some(n) = p.file_stem().map(|s| s.to_string_lossy().to_string()) {
            let limpo: String = n
                .chars()
                .map(|c| {
                    if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                        ' '
                    } else {
                        c
                    }
                })
                .collect();
            let limpo = limpo.split_whitespace().collect::<Vec<_>>().join(" ");
            if !limpo.is_empty() {
                stem = limpo;
            }
        }
        mime = mime_of_path(&p);
        std::fs::read(&p).map_err(|e| e.to_string())?
    } else if let Some(arquivo) = caminho_do_texto(entry) {
        // O Windows copia arquivos como `CF_HDROP` **e** texto com o caminho;
        // quando só o texto fica guardado, o arrasto ainda tem que entregar o
        // arquivo (era o caso do `.png` que virava `.txt`).
        if let Some(e) = arquivo.extension().map(|e| e.to_string_lossy().to_lowercase()) {
            if !e.is_empty() {
                ext = e;
            }
        }
        if let Some(n) = arquivo.file_stem().map(|s| s.to_string_lossy().to_string()) {
            let limpo: String = n
                .chars()
                .map(|c| {
                    if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                        ' '
                    } else {
                        c
                    }
                })
                .collect();
            let limpo = limpo.split_whitespace().collect::<Vec<_>>().join(" ");
            if !limpo.is_empty() {
                stem = limpo;
            }
        }
        mime = mime_of_path(&arquivo);
        std::fs::read(&arquivo).map_err(|e| e.to_string())?
    } else {
        let t = texto_do_entry(entry);
        let mut corte = t.len().min(MAX_ARRASTO_TEXTO);
        while corte > 0 && !t.is_char_boundary(corte) {
            corte -= 1;
        }
        texto = Some(t[..corte].to_string());
        t.into_bytes()
    };

    if bytes.len() as u64 > MAX_ARRASTO {
        return Err("conteudo grande demais para arrastar".into());
    }
    let link = entry.kind == "link";
    crate::log(&format!("drag payload: {stem}.{ext} {mime} {} bytes", bytes.len()));
    Ok(DragPayload {
        name: format!("{stem}.{ext}"),
        data: b64_encode(&bytes),
        mime,
        text: texto,
        link,
    })
}

/// Caminho de arquivo escondido num texto ("C:\pasta\foto.png", com ou sem
/// aspas). Só vale para uma linha que exista no disco.
fn caminho_do_texto(entry: &crate::store::Entry) -> Option<std::path::PathBuf> {
    let texto = entry.text.as_deref()?.trim();
    if texto.is_empty() || texto.lines().count() > 1 {
        return None;
    }
    let limpo = texto.trim_matches('"').trim_matches('\'').trim();
    if limpo.len() < 4 || !limpo.contains(':') {
        return None;
    }
    let p = std::path::PathBuf::from(limpo);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Corpo de texto da entrada (HTML/RTF preferidos, texto como último recurso).
fn texto_do_entry(entry: &crate::store::Entry) -> String {
    entry
        .text
        .clone()
        .or_else(|| entry.ocr_text.clone())
        .unwrap_or_default()
}

/// Tipo MIME a partir da extensão do arquivo.
fn mime_of_path(p: &std::path::Path) -> String {
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "txt" | "log" | "md" | "csv" => "text/plain",
        "json" => "application/json",
        "zip" => "application/zip",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// O arrasto começou de verdade (o `dragstart` do Chromium aceitou o payload).
#[tauri::command]
fn drag_start() {
    crate::log("drag: comecou");
    ARRASTANDO.store(true, Ordering::SeqCst);
}

/// O frontend avisa quando o arrasto terminou — enquanto durar, a janela não
/// pode se esconder (o foco vai para o Explorer e o app se fecharia no meio).
#[tauri::command]
fn drag_end() {
    ARRASTANDO.store(false, Ordering::SeqCst);
}



#[tauri::command]
fn get_settings(state: State<AppState>) -> settings::Settings {
    lock_settings(&state.settings).clone()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: settings::Settings,
) -> Result<Vec<String>, String> {
    let inicio = std::time::Instant::now();
    // Estado **anterior** (o que está valendo), para aplicar só o que mudou.
    let anterior = lock_settings(&state.settings).clone();
    let mut cfg = settings.sanitized();
    cfg.save(&app)?;
    *lock_settings(&state.settings) = cfg.clone();
    let warnings = apply_settings(&app, Some(&anterior), &mut cfg);
    // Se algum atalho foi revertido (já usado por outro app), o valor mantido
    // tem que ir para o disco — senão a próxima abertura tentaria de novo o
    // atalho impossível e o app ficaria sem atalho de novo.
    if cfg.show_hotkey != anterior.show_hotkey || cfg.paste_hotkey != anterior.paste_hotkey {
        cfg.save(&app)?;
        *lock_settings(&state.settings) = cfg.clone();
    }
    // Tempo da gravação a partir daqui: `save_settings` que passa de ~100 ms é
    // sinal de que algum passo caro voltou a rodar sem necessidade.
    log(&format!(
        "settings saved in {} ms ({} avisos)",
        inicio.elapsed().as_millis(),
        warnings.len()
    ));
    // Avisa a UI (tema, cores e toggles aplicados na hora).
    let _ = app.emit("settings:changed", ());
    Ok(warnings)
}

/// Reinicia o Explorer para o `DisabledHotkeys` (Win+V) valer.
#[tauri::command]
fn restart_explorer() -> Result<(), String> {
    sysinteg::restart_explorer()
}

/// Liga/desliga os "efeitos de transparência" do Windows (necessário para o
/// fundo translúcido aparecer). Só mexe em HKCU, o usuário pode reverter.
#[tauri::command]
fn set_windows_transparency(
    app: AppHandle,
    state: State<AppState>,
    enable: bool,
) -> Result<bool, String> {
    sysinteg::set_windows_transparency(enable)?;
    let translucent = { lock_settings(&state.settings).translucent };
    if let Some(win) = app.get_webview_window("main") {
        if let Ok(h) = win.hwnd() {
            let escuro = app
                .try_state::<AppState>()
                .map(|st| st.settings.lock().unwrap_or_else(|e| e.into_inner()).theme != "light")
                .unwrap_or(true);
            win32::apply_backdrop(h.0 as isize, if translucent && enable { 2 } else { 1 }, escuro);
        }
    }
    Ok(sysinteg::windows_transparency())
}

/// Cache do estado do Windows: `reg query` + `schtasks` custam ~100 ms e eram
/// executados a cada montagem da interface. 30 s de validade; a página de
/// configurações pede `force` depois de cada alteração.
static SYS_CACHE: std::sync::Mutex<Option<(u64, serde_json::Value)>> =
    std::sync::Mutex::new(None);

fn agora_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Consulta a última release do GitHub.
///
/// `force` ignora o cache de 6 h (usado pelo botão "Check now"); a consulta
/// silenciosa de fundo usa o cache para não gastar o limite de 60 req/h da API
/// anônima do GitHub.
#[tauri::command]
async fn update_check(
    app: AppHandle,
    force: Option<bool>,
) -> Result<Option<update::UpdateInfo>, String> {
    if lock_settings(&app.state::<AppState>().settings).offline_mode {
        return Err("offline mode is on".to_string());
    }
    let force = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || update::check(force))
        .await
        .map_err(|e| e.to_string())?
}

/// Baixa, confere e troca o exe, e reabre o app na versão nova.
///
/// O processo novo espera este sair (dois processos com o banco aberto ao
/// mesmo tempo é o que estraga o histórico), então este sai logo depois de
/// responder à interface.
#[tauri::command]
async fn update_install(app: AppHandle) -> Result<String, String> {
    if lock_settings(&app.state::<AppState>().settings).offline_mode {
        return Err("offline mode is on".to_string());
    }
    // Um install por vez: dois cliques (ou um clique e o vigia de versão) não
    // podem baixar e trocar o exe ao mesmo tempo.
    if INSTALANDO.swap(true, Ordering::SeqCst) {
        return Err("an update is already running".to_string());
    }
    struct Solta;
    impl Drop for Solta {
        fn drop(&mut self) {
            INSTALANDO.store(false, Ordering::SeqCst);
        }
    }
    let _solta = Solta;
    let exe = update::exe_atual().ok_or("no exe path")?;
    let emit_handle = app.clone();
    let versao = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let info = update::check(true)?
            .ok_or_else(|| "you are already on the latest version".to_string())?;
        // Fase visível na interface: baixando -> trocando.
        let _ = emit_handle.emit("update:phase", "download");
        let emit_progresso = emit_handle.clone();
        update::instalar(&info, &exe, move |baixado, total| {
            let _ = emit_progresso.emit(
                "update:progress",
                serde_json::json!({ "downloaded": baixado, "total": total }),
            );
        })?;
        let _ = emit_handle.emit("update:phase", "swap");
        update::relaunch(&exe)?;
        Ok(info.version)
    })
    .await
    .map_err(|e| e.to_string())??;
    let h = app.clone();
    std::thread::spawn(move || {
        // Folga para a resposta chegar à interface antes de fechar.
        std::thread::sleep(Duration::from_millis(900));
        h.exit(0);
    });
    Ok(versao)
}

/// Estado real do Windows: o Win+V nativo está desativado?
#[tauri::command]
fn system_status(state: State<AppState>, force: Option<bool>) -> serde_json::Value {
    let force = force.unwrap_or(false);
    if !force {
        if let Ok(g) = SYS_CACHE.lock() {
            if let Some((t, v)) = g.as_ref() {
                if agora_ms().saturating_sub(*t) < 30_000 {
                    return v.clone();
                }
            }
        }
    }
    let (encrypted_db, db_path, key_error) = {
        let store = lock_store(&state.store);
        (
            store.encrypted,
            store.dir.join(store::DB_FILE).to_string_lossy().to_string(),
            store.key_error.clone(),
        )
    };
    let valor = serde_json::json!({
        "winVDisabled": sysinteg::replace_win_v_active(),
        "startupTask": sysinteg::startup_task_active(),
        "elevated": sysinteg::is_elevated(),
        "winTransparency": sysinteg::windows_transparency(),
        "encrypted": encrypted_db,
        "keyError": key_error,
        "version": env!("CARGO_PKG_VERSION"),
        "dbPath": db_path,
        "exe": std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
    });
    if let Ok(mut g) = SYS_CACHE.lock() {
        *g = Some((agora_ms(), valor.clone()));
    }
    valor
}

/// Lock tolerante a **envenenamento**: se outra thread sofrer panic segurando o
/// mutex, `librar()` normal faria todo comando seguinte em pânico — o app inteiro
/// morreria. Aqui recuperamos o conteúdo e seguimos.
/// Quanto de texto vai para a lista (busca + prévia). O resto é buscado sob
/// demanda por `entry_text`, no painel de detalhes.
const LIST_TEXT_MAX: usize = 4 * 1024;

/// Corta uma string em `max` **bytes** sem quebrar caractere UTF-8.
/// Versão pública do corte por caracteres (usada por `search.rs`).
pub fn cut_utf8_pub(s: &str, max: usize) -> &str {
    cut_utf8(s, max)
}

fn cut_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Entradas antigas guardavam favicon e capa como data URL: alguns KB (a capa,
/// centenas) viajavam em **toda** listagem. Aqui eles viram arquivos e a
/// entrada passa a apontar para `http://rayclip.localhost/fav|cover/...`.
fn migra_links_para_arquivos(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let mut mudou: Vec<Entry> = Vec::new();
    {
        let mut store = lock_store(&state.store);
        let media = store.media_dir();
        let _ = std::fs::create_dir_all(media.join("fav"));
        let _ = std::fs::create_dir_all(media.join("covers"));
        for e in store.entries.iter_mut() {
            let Some(link) = e.link.as_mut() else { continue };
            let url = link
                .url
                .clone()
                .or_else(|| e.text.clone())
                .unwrap_or_default();
            if let Some(host) = host_of_url(&url) {
                if let Some(data) = link.icon.as_deref().filter(|i| i.starts_with("data:")) {
                    if let Some(bytes) = dec_data_url(data) {
                        let p = media.join("fav").join(&host);
                        if !p.exists() {
                            let _ = std::fs::write(&p, bytes);
                        }
                        // Mesmo que o arquivo já existisse (host repetido em
                        // várias entradas), a entrada passa a apontar para ele.
                        if p.exists() {
                            link.icon = Some(format!("http://rayclip.localhost/fav/{host}"));
                        }
                    }
                }
            }
            if let Some(data) = link
                .image_path
                .as_deref()
                .filter(|i| i.starts_with("data:"))
            {
                if let Some(bytes) = dec_data_url(data) {
                    if let Some(key) = linkmeta::cover_key(&format!("{url}#cover")) {
                        let p = media.join("covers").join(&key);
                        if !p.exists() {
                            let _ = std::fs::write(&p, bytes);
                        }
                        if p.exists() {
                            link.image_path =
                                Some(format!("http://rayclip.localhost/cover/{key}"));
                        }
                    }
                }
            }
            if let Some(l) = e.link.as_ref() {
                if l.icon.as_deref().is_some_and(|i| i.starts_with("data:"))
                    || l.image_path.as_deref().is_some_and(|i| i.starts_with("data:"))
                {
                    continue;
                }
            }
            mudou.push(e.clone());
        }
        for e in &mudou {
            store.touch(e);
        }
    }
    if !mudou.is_empty() {
        log(&format!("migracao: {} links com imagem em arquivo", mudou.len()));
        notify_changed(app);
    }
}

/// `data:image/png;base64,AAAA` -> bytes.
fn dec_data_url(s: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let b64 = s.split(',').nth(1)?;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

fn host_of_url(u: &str) -> Option<String> {
    let rest = u.split("://").nth(1).unwrap_or(u);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host)
}

/// Uma colagem por vez: o Raycast também serializa (`_textPasteLock`), senão
/// duas colagens seguidas disputam o foco e o alvo recebe texto trocado.
static PASTE_LOCK: Mutex<()> = Mutex::new(());

fn lock_paste() -> std::sync::MutexGuard<'static, ()> {
    PASTE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Cola, e se o primeiro `SendInput` não saiu com o alvo na frente, tenta de
/// novo (o Raycast faz o mesmo com `MaxPasteAttempts`).
fn paste_with_retry(target: isize) {
    for tentativa in 1..=2 {
        if win32::paste_into(target) {
            return;
        }
        log(&format!("paste_into: tentativa {tentativa} falhou, repetindo"));
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn lock_store(m: &Mutex<Store>) -> std::sync::MutexGuard<'_, Store> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock_settings(m: &Mutex<settings::Settings>) -> std::sync::MutexGuard<'_, settings::Settings> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock_shown_at(m: &Mutex<std::time::Instant>) -> std::sync::MutexGuard<'_, std::time::Instant> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock_ignores(m: &Mutex<Vec<u32>>) -> std::sync::MutexGuard<'_, Vec<u32>> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A janela está na tela? Rastreamos nós mesmos: `IsWindowVisible` retorna
/// `true` desde a criação (o Tauri mapeia a janela mesmo com `visible: false`),
/// o que furava qualquer decisão de "mostrar/esconder".
static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Um arrasto para fora está em andamento (a janela não esconde).
static ARRASTANDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Vigia do renderizador (§8.43). O WebView2 pode morrer (processo do renderer
/// morto por falta de memória, driver de vídeo, crash) e a janela fica branca
/// para sempre. Raycast 2.8 recarrega nesse caso; fazemos igual.
///
/// Como se detecta: a página manda `heartbeat` a cada 3 s. Se a janela está na
/// tela há mais de 20 s e não chega batida há 15 s, o renderer não está
/// executando JS — recarrega.
static ULTIMO_BATIMENTO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Um `update_install` por vez (dois cliques não podem trocar o exe juntos).
static INSTALANDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Quantos `reload()` já foram tentados sem a página voltar a dar sinal.
static TENTATIVAS_RELOAD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[tauri::command]
fn heartbeat() {
    ULTIMO_BATIMENTO.store(agora_ms(), Ordering::SeqCst);
    if TENTATIVAS_RELOAD.load(Ordering::SeqCst) > 0 {
        log("renderer voltou a responder heartbeat");
    }
    TENTATIVAS_RELOAD.store(0, Ordering::SeqCst);
}


fn set_shown(v: bool) {
    SHOWN.store(v, Ordering::SeqCst);
}

/// Avisa a UI que o histórico mudou. Com a janela escondida o frontend não
/// precisa da lista (que tem alguns MB): ao mostrar, `clipboard:focus` já
/// recarrega. Sem isso, cada cópia custava ~1 MB de IPC à toa.
fn notify_changed(app: &AppHandle) {
    let vis = visible_now(app);
    if cfg!(debug_assertions) {
        log(&format!("notify: janela visivel={vis}"));
    }
    if vis {
        let _ = app.emit("clipboard:changed", ());
    }
}

fn entries_now(state: &AppState) -> Vec<Entry> {
    let t0 = std::time::Instant::now();
    let store = lock_store(&state.store);
    // Projeção explícita: clonamos só o que a UI usa. `store.entries.clone()`
    // copiava também `html`/`rtf` (34 MB no histórico importado) **segurando o
    // lock** — 112 ms por listagem e captura travada enquanto isso.
    let mut list: Vec<Entry> = store
        .entries
        .iter()
        .map(|e| Entry {
            id: e.id.clone(),
            kind: e.kind.clone(),
            title: e.title.clone(),
            text: e
                .text
                .as_deref()
                .map(|t| cut_utf8(t, LIST_TEXT_MAX).to_string()),
            paths: e.paths.clone(),
            has_image: e.has_image,
            image_width: e.image_width,
            image_height: e.image_height,
            source: e.source.clone(),
            source_icon: e.source_icon.clone(),
            ocr_text: e
                .ocr_text
                .as_deref()
                .map(|t| cut_utf8(t, LIST_TEXT_MAX).to_string()),
            has_thumb: e.has_thumb,
            gif: e.gif,
            link: e.link.clone(),
            html: None,
            rtf: None,
            size_bytes: e.size_bytes,
            created_at: e.created_at,
            pinned: e.pinned,
            pin_order: e.pin_order,
            hash: e.hash.clone(),
        })
        .collect();
    list.sort_by(|a, b| {
        // Fixados primeiro — e, entre eles, na ordem escolhida pelo usuário
        // ("Move Up/Down in Pinned"); os demais por recência.
        b.pinned.cmp(&a.pinned).then_with(|| {
            if a.pinned {
                a.pin_order.cmp(&b.pin_order)
            } else {
                b.created_at.cmp(&a.created_at)
            }
        })
    });
    drop(store); // o lock já pode ir embora: o resto é só CPU local.
    if cfg!(debug_assertions) {
        // Mede o tamanho do que trafega por IPC a cada mudança do histórico.
        let ms = t0.elapsed().as_millis();
        let t = std::time::Instant::now();
        let bytes = serde_json::to_string(&list).map(|s| s.len()).unwrap_or(0);
        log(&format!(
            "list: {} entradas, {} KB, projecao {ms}ms, serializado em {:?}",
            list.len(),
            bytes / 1024,
            t.elapsed()
        ));
    }
    list
}

fn run_ocr_for(app: AppHandle, id: String) {
    if !lock_settings(&app.state::<AppState>().settings).ocr_enabled {
        return;
    }
    std::thread::spawn(move || {
        // Indexar imagem é trabalho de fundo: fica abaixo do normal para não
        // roubar CPU da janela enquanto o usuário rola o histórico.
        win32::thread_below_normal();
        let Some(_slot) = acquire_job(20_000) else {
            return;
        };
        ocr_one(&app, &id);
    });
}

/// Indexa uma imagem (quem chama já segura uma vaga do pool).
fn ocr_one(app: &AppHandle, id: &str) -> bool {
    // Windows sem o componente de OCR: não marca nada como "já tentado" (a
    // imagem continua pendente e volta a ser indexada se o componente for
    // instalado). Antes isto virava `Some("")` e a imagem ficava "sem texto"
    // para sempre.
    if !ocr::disponivel() {
        return false;
    }
    let bytes = {
        let state = app.state::<AppState>();
        let store = lock_store(&state.store);
        let has_image = store.get(id).is_some_and(|e| e.has_image);
        if !has_image {
            return false;
        }
        match store.image_bytes(id) {
            Some(b) => b,
            None => return false,
        }
    };
    let Some(text) = ocr::image_text_bytes(&bytes) else {
        // Marca como "já tentado" (texto vazio) para não reindexar a mesma
        // imagem em todo início do app — cada tentativa custa segundos.
        {
            let state = app.state::<AppState>();
            let mut store = lock_store(&state.store);
            if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                e.ocr_text = Some(String::new());
                let e = e.clone();
                store.touch(&e);
            }
        }
        log(&format!("ocr: no text in {id}"));
        return true;
    };
    let text: String = text.chars().take(20_000).collect();
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state.store);
        if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
            e.ocr_text = Some(text);
            let e = e.clone();
            store.touch(&e);
        }
    }
    log(&format!("ocr: indexed {id}"));
    notify_changed(app);
    true
}

/// Backfill do histórico importado: uma única thread, item por item, sem
/// desistir. Antes eram 700+ threads disputando 2 vagas e desistindo em 20 s,
/// então o índice nunca terminava e o mesmo trabalho recomeçava a cada início.
fn start_ocr_backfill(app: AppHandle, ids: Vec<String>) {
    if ids.is_empty() || !lock_settings(&app.state::<AppState>().settings).ocr_enabled {
        return;
    }
    std::thread::spawn(move || {
        for id in ids {
            // Com a janela aberta, cede a CPU: indexar não pode travar a UI.
            while visible_now(&app) {
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            let Some(_slot) = acquire_job(600_000) else {
                return;
            };
            ocr_one(&app, &id);
        }
        log("ocr: backfill concluido");
    });
}

/// Indexa com OCR todas as imagens que ainda não têm texto reconhecido.
// ---- Limite de trabalhos de indexação em paralelo (OCR/link/miniatura) ------
static JOBS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct JobGuard;
impl Drop for JobGuard {
    fn drop(&mut self) {
        JOBS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Pega uma vaga (no máximo 2 trabalhos pesados ao mesmo tempo). Espera até `ms`.
fn acquire_job(ms: u64) -> Option<JobGuard> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        if JOBS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 {
            return Some(JobGuard);
        }
        JOBS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

/// Miniatura real de arquivo (PDF, vídeo, doc) — gerada só quando o item é
/// selecionado, nunca para o histórico inteiro.
#[tauri::command]
fn thumb_entry(app: AppHandle, _state: State<AppState>, id: String) {
    std::thread::spawn(move || {
        let Some(_slot) = acquire_job(8_000) else {
            return;
        };
        let st = app.state::<AppState>();
        let path = {
            let store = lock_store(&st.store);
            let Some(e) = store.get(&id) else { return };
            if e.has_thumb {
                return;
            }
            if !matches!(
                e.kind.as_str(),
                "file" | "files" | "pdf" | "video" | "audio"
            ) {
                return;
            }
            let Some(p) = e.paths.first().cloned() else {
                return;
            };
            if !std::path::Path::new(&p).is_file() {
                return;
            }
            p
        };
        let tmp = std::env::temp_dir().join(format!(
            "rayclip-thumb-{}-{}.png",
            linkmeta::short_hash(&path),
            std::process::id()
        ));
        let Some(_png) = thumbs::file_thumb(std::path::Path::new(&path), &tmp, 512) else {
            log(&format!("thumb: sem miniatura para {path}"));
            return;
        };
        let bytes = std::fs::read(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        if bytes.is_empty() {
            return;
        }
        {
            let state = app.state::<AppState>();
            let mut store = lock_store(&state.store);
            store.put_thumb(&id, &bytes);
        }
        log(&format!("thumb: {path}"));
        notify_changed(&app);
    });
}

/// Busca os metadados do site para uma entrada de link (uma única vez).
fn run_link_for(app: AppHandle, id: String, forcar: bool) {
    let state = app.state::<AppState>();
    let (links_on, offline) = {
        let cfg = lock_settings(&state.settings);
        (cfg.link_previews, cfg.offline_mode)
    };
    // "Offline & private" manda desligar qualquer requisição de rede.
    if !links_on || offline {
        return;
    }
    std::thread::spawn(move || {
        let Some(_slot) = acquire_job(30_000) else {
            return;
        };
        let url = {
            let state = app.state::<AppState>();
            let store = lock_store(&state.store);
            match store.get(&id) {
                Some(e) if e.kind == "link" => e.text.clone().unwrap_or_else(|| e.title.clone()),
                _ => return,
            }
        };
        if url.trim().is_empty() {
            log("link: url vazia");
            return;
        }
        // Já tem metadados? Só resolve o ícone (o cache por host evita rede).
        // Exceção: metadados sem título de página (site bloqueou, página só de
        // JS) ainda não tentados — esses valem uma nova ida à rede, senão a
        // linha fica para sempre mostrando a URL crua.
        let (media, tem_meta, falta_titulo) = {
            let state = app.state::<AppState>();
            let store = lock_store(&state.store);
            let entry = store.get(&id);
            let falta = entry
                .as_ref()
                .and_then(|e| e.link.as_ref())
                .map(|l| {
                    // Falta título **ou** capa conta como incompleto: uma prévia
                    // sem imagem é justamente o que o usuário vê como errado.
                    let sem_titulo = l
                        .title
                        .as_deref()
                        .map(linkmeta::titulo_ruim)
                        .unwrap_or(true);
                    let sem_capa = l.image_path.is_none();
                    (sem_titulo || sem_capa)
                        && (!l.tried
                            || l.fetch_v.unwrap_or(0) < linkmeta::FETCH_VERSION)
                })
                .unwrap_or(false);
            (
                store.media_dir(),
                entry.is_some_and(|e| e.link.is_some()),
                falta,
            )
        };
        if tem_meta && !falta_titulo && !forcar {
            if let Some(icon) = linkmeta::favicon_url(&media, &url) {
                let state = app.state::<AppState>();
                let mut store = lock_store(&state.store);
                if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                    if let Some(l) = e.link.as_mut() {
                        l.icon = Some(icon);
                    }
                    let e = e.clone();
                    store.touch(&e);
                }
                log(&format!("link: icone de {url}"));
                notify_changed(&app);
            }
            return;
        }
        log(&format!("link: buscando {url}"));
        let media = {
            let state = app.state::<AppState>();
            let store = lock_store(&state.store);
            store.media_dir()
        };
        // Página inacessível (403, timeout) não deixa a linha genérica: fica com
        // o host e o favicon do domínio, e não é mais re-tentada a cada abertura.
        let meta = linkmeta::fetch(&url, &media).unwrap_or_else(|| {
            log(&format!("link: sem metadados para {url} (minimo)"));
            linkmeta::minimal(&url, &media)
        });
        {
            let state = app.state::<AppState>();
            let mut store = lock_store(&state.store);
            let mut touched: Vec<Entry> = Vec::new();
            for e in store.entries.iter_mut() {
                let same_id = e.id == id;
                let same_url = e.kind == "link"
                    && e.link.is_none()
                    && e.text.as_deref().map(str::trim) == Some(url.as_str());
                if same_id || same_url {
                    e.link = Some(meta.clone());
                    touched.push(e.clone());
                }
            }
            for e in &touched {
                store.touch(e);
            }
        }
        log(&format!("link: metadados de {url}"));
        notify_changed(&app);
    });
}

/// Metadados de uma página que **só um navegador de verdade** lê.
///
/// Chamado pela interface quando o usuário abre uma entrada de link que ficou
/// sem capa (klipy.com e afins respondem 403 "Just a moment…" a qualquer outro
/// cliente). Roda numa thread: abre o WebView2 invisível, pega as URLs e baixa
/// a capa. Devolve `true` se começou.
#[tauri::command]
fn link_web_meta(app: AppHandle, state: State<AppState>, id: String) -> Result<bool, String> {
    let (url, precisa, media) = {
        let store = lock_store(&state.store);
        let Some(e) = store.get(&id) else {
            return Err("entry not found".into());
        };
        let link = e.link.as_ref();
        let url = link
            .and_then(|l| l.url.clone())
            .or_else(|| e.text.clone())
            .unwrap_or_default();
        let permissao = url.starts_with("http://") || url.starts_with("https://");
        // Só vale a pena quando falta a capa (título ruim sozinho não paga o
        // custo de um render).
        let falta_capa = link.map(|l| l.image_path.is_none()).unwrap_or(true);
        (url, permissao && falta_capa, store.media_dir())
    };
    if !precisa || url.is_empty() {
        return Ok(false);
    }
    {
        let cfg = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        // "Offline & private": nenhuma requisição automática, nem esta.
        if cfg.offline_mode {
            return Ok(false);
        }
    }
    if webmeta::ocupado() {
        return Ok(false);
    }
    std::thread::spawn(move || {
        let Some(candidatas) = webmeta::buscar(&app, &url) else {
            log(&format!("webmeta: nada em {url}"));
            return;
        };
        log(&format!(
            "webmeta: {} candidata(s) de {url}",
            candidatas.len()
        ));
        let Some((capa, video)) = linkmeta::baixar_capa(&media, &url, &candidatas) else {
            log(&format!("webmeta: nenhuma capa baixou de {url}"));
            return;
        };
        let state = app.state::<AppState>();
        {
            let mut store = lock_store(&state.store);
            let mut pronto: Option<Entry> = None;
            for e in store.entries.iter_mut().filter(|e| e.id == id) {
                if let Some(l) = e.link.as_mut() {
                    l.image_path = Some(capa.clone());
                    l.cover_video = video;
                    l.fetch_v = Some(linkmeta::FETCH_VERSION);
                    l.tried = true;
                    pronto = Some(e.clone());
                }
            }
            let Some(e) = pronto else { return };
            store.touch(&e);
        }
        log(&format!("webmeta: capa de {url} ({})", if video { "video" } else { "imagem" }));
        notify_changed(&app);
    });
    Ok(true)
}

/// Quantos links ainda não têm metadados (indexa os antigos ao abrir).
#[tauri::command]
fn link_pending(app: AppHandle, state: State<AppState>) -> usize {
    // Duas filas: (1) quem só precisa do ícone (rápido, cache por host);
    // (2) quem nunca teve metadados (vai à rede, com timeout). Antes, uma fila
    // única cheia de URLs bloqueadas consumia as vagas e nunca chegava ao ícone.
    let (so_icone, sem_meta, sem_titulo): (Vec<String>, Vec<String>, Vec<String>) = {
        let store = lock_store(&state.store);
        let mut a = Vec::new();
        let mut b = Vec::new();
        let mut c = Vec::new();
        for e in store.entries.iter().filter(|e| e.kind == "link") {
            match &e.link {
                Some(l) if l.icon.is_none() => a.push(e.id.clone()),
                Some(l) => {
                    let sem = l
                        .title
                        .as_deref()
                        .map(linkmeta::titulo_ruim)
                        .unwrap_or(true);
                    let sem_capa = l.image_path.is_none();
                    if (sem || sem_capa)
                        && (!l.tried || l.fetch_v.unwrap_or(0) < linkmeta::FETCH_VERSION)
                    {
                        c.push(e.id.clone());
                    }
                }
                None => b.push(e.id.clone()),
            }
        }
        a.truncate(24);
        b.truncate(6);
        c.truncate(4);
        (a, b, c)
    };
    let ids: Vec<String> = so_icone
        .into_iter()
        .chain(sem_meta)
        .chain(sem_titulo)
        .collect();
    let n = ids.len();
    for id in ids {
        run_link_for(app.clone(), id, false);
    }

    // Segunda passada: links que ficaram **sem capa** (download recusado, 429,
    // site lento). Cada um é tentado uma vez por execução do app — assim uma
    // prévia vazia se conserta sozinha sem virar tráfego a cada abertura.
    if n == 0 {
        let pendentes: Vec<String> = {
            let store = lock_store(&state.store);
            let mut ja = capa_tentadas().lock().unwrap_or_else(|e| e.into_inner());
            let mut v: Vec<String> = Vec::new();
            for e in store.entries.iter().filter(|e| e.kind == "link") {
                if e.link.as_ref().is_some_and(|l| l.image_path.is_none())
                    && !ja.contains(&e.id)
                {
                    ja.insert(e.id.clone());
                    v.push(e.id.clone());
                }
            }
            v.truncate(3);
            v
        };
        for id in pendentes {
            run_link_for(app.clone(), id, true);
        }
    }
    n
}

/// Ids que já tiveram a capa re-tentada nesta execução do app.
fn capa_tentadas() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    SET.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

#[tauri::command]
fn link_entry(app: AppHandle, state: State<AppState>, id: String) -> bool {
    let ok = {
        let store = lock_store(&state.store);
        store.get(&id).map(|e| e.kind == "link").unwrap_or(false)
    };
    if ok {
        run_link_for(app, id, true);
    }
    ok
}

#[tauri::command]
fn ocr_pending(app: AppHandle, state: State<AppState>) -> usize {
    if !ocr::disponivel() {
        log("ocr_pending: motor de OCR indisponivel neste Windows");
        return 0;
    }
    let ids: Vec<String> = {
        let store = lock_store(&state.store);
        store
            .entries
            .iter()
            .filter(|e| e.kind == "image" && e.ocr_text.is_none())
            .map(|e| e.id.clone())
            .collect()
    };
    let total = ids.len();
    if cfg!(debug_assertions) {
        log(&format!("ocr_pending: {total} imagens sem texto"));
    }
    start_ocr_backfill(app, ids);
    total
}

#[tauri::command]
fn ocr_entry(app: AppHandle, id: String) {
    run_ocr_for(app, id);
}

/// A interface só pode dizer "nenhum texto" quando o Windows **tem** OCR.
/// Sem o componente, a resposta honesta é que o recurso não existe aqui.
#[tauri::command]
fn ocr_disponivel() -> bool {
    ocr::disponivel()
}

#[tauri::command]
fn copy_ocr_text(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let text = {
        let store = lock_store(&state.store);
        store.get(&id).and_then(|e| e.ocr_text.clone())
    }
    .ok_or_else(|| "no OCR text".to_string())?;
    let _guard = clipboard_win::Clipboard::new_attempts(6).map_err(|e| e.to_string())?;
    clipboard_win::formats::Unicode
        .write_clipboard(&text)
        .map_err(|e| e.to_string())?;
    notify_changed(&app);
    Ok(())
}

/// Quantos itens existem no histórico do Raycast (para o botão do Settings).
#[tauri::command]
fn raycast_import_count() -> usize {
    import::default_dir()
        .map(|d| import::count(&d))
        .unwrap_or(0)
}

/// Importa o histórico do Raycast para o banco cifrado do Rayclip.
#[tauri::command]
fn import_raycast(
    app: AppHandle,
    state: State<AppState>,
    path: Option<String>,
) -> Result<usize, String> {
    let dir = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => import::default_dir().ok_or_else(|| {
            "Raycast history not found (%LOCALAPPDATA%\\Raycast\\clipboard)".to_string()
        })?,
    };
    if !dir.is_dir() {
        return Err("folder not found".into());
    }
    // Se o histórico do Raycast for maior que o limite atual, aumenta o limite
    // (senão a importação seria cortada na hora).
    let need = import::count(&dir);
    {
        let mut store = lock_store(&state.store);
        if !store.encrypted {
            return Err("encrypted database is not available".into());
        }
        let mut cfg = lock_settings(&state.settings).clone();
        if need > store.max_entries {
            let target = [100usize, 300, 500, 1000, 2000, 5000]
                .into_iter()
                .find(|n| *n >= need)
                .unwrap_or(5000);
            cfg.max_entries = target;
            store.max_entries = target;
            if let Err(e) = cfg.save(&app) {
                log(&format!("import: nao salvou o limite novo: {e}"));
            } else {
                log(&format!("import: limite do historico -> {target}"));
                let _ = app.emit("settings:changed", ());
            }
            *lock_settings(&state.settings) = cfg;
        }
    }
    let handle = app.clone();
    let total = {
        let mut store = lock_store(&state.store);
        let mut last = 0usize;
        let mut progress = |done: usize, total: usize| {
            if done == total || done % 100 == 0 {
                log(&format!("import: {done}/{total}"));
            }
            last = total;
        };
        import::run(&handle, &mut store, &dir, &mut progress);
        last
    };
    notify_changed(&app);
    Ok(total)
}

#[tauri::command]
fn list_entries(state: State<AppState>) -> Vec<Entry> {
    entries_now(&state)
}

/// Filtro + busca + paginação **no Rust** (§8.44).
///
/// Antes a página levava todas as entradas (1,8 MB, ~33 ms de serialização a
/// cada atualização) e filtrava em JS. Aqui só sai o que vai aparecer, e a
/// projeção (`html`/`rtf` fora, texto cortado) é feita apenas nos casados.
#[tauri::command]
fn search_entries(
    state: State<AppState>,
    query: Option<String>,
    filter: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<serde_json::Value, String> {
    let t0 = std::time::Instant::now();
    let q = query.unwrap_or_default();
    let f = filter.unwrap_or_else(|| "all".to_string());
    let off = offset.unwrap_or(0);
    let lim = limit.unwrap_or(200).clamp(1, 2000);

    let store = lock_store(&state.store);
    let db = store.db().ok_or_else(|| "banco nao esta aberto".to_string())?;
    // Ordenação de origem (fixados na mão, depois recência) vinda do banco.
    let mut base: Vec<Entry> = store
        .entries
        .iter()
        .filter(|e| search::matches_filter(e, &f))
        .map(projeta_entry)
        .collect();
    drop(db);
    drop(store);

    base.sort_by(|a, b| {
        b.pinned.cmp(&a.pinned).then_with(|| {
            if a.pinned {
                a.pin_order.cmp(&b.pin_order)
            } else {
                b.created_at.cmp(&a.created_at)
            }
        })
    });

    let (itens, total) = search::filtrar(base, &q, &f, off, lim);
    let bytes = serde_json::to_string(&itens).map(|s| s.len()).unwrap_or(0);
    log(&format!(
        "busca: q={:?} filtro={} offset={} -> {} de {} itens, {} KB, {} ms",
        q,
        f,
        off,
        itens.len(),
        total,
        bytes / 1024,
        t0.elapsed().as_millis()
    ));
    Ok(serde_json::json!({
        "items": itens,
        "total": total,
        "ms": t0.elapsed().as_millis(),
    }))
}

/// Projeção enxuta de uma entrada para a lista (a mesma do `entries_now`).
fn projeta_entry(e: &Entry) -> Entry {
    Entry {
        id: e.id.clone(),
        kind: e.kind.clone(),
        title: e.title.clone(),
        text: e
            .text
            .as_deref()
            .map(|t| cut_utf8(t, LIST_TEXT_MAX).to_string()),
        paths: e.paths.clone(),
        has_image: e.has_image,
        image_width: e.image_width,
        image_height: e.image_height,
        source: e.source.clone(),
        source_icon: e.source_icon.clone(),
        ocr_text: e
            .ocr_text
            .as_deref()
            .map(|t| cut_utf8(t, LIST_TEXT_MAX).to_string()),
        has_thumb: e.has_thumb,
        gif: e.gif,
        link: e.link.clone(),
        html: None,
        rtf: None,
        size_bytes: e.size_bytes,
        created_at: e.created_at,
        pinned: e.pinned,
        pin_order: e.pin_order,
        hash: e.hash.clone(),
    }
}

/// Compacta o banco (`VACUUM`): reescreve o arquivo sem as páginas livres que
/// sobraram de exclusões.
///
/// Isso devolve **espaço em disco** — não é "apagar para sempre". Com
/// `secure_delete = ON` as páginas apagadas já foram zeradas; o que sobra é
/// arquivo grande demais, não conteúdo.
#[tauri::command]
fn compact_db(state: State<AppState>) -> Result<serde_json::Value, String> {
    let caminho = {
        let store = lock_store(&state.store);
        store.db_file()
    };
    let antes = std::fs::metadata(&caminho).map(|m| m.len()).unwrap_or(0);
    {
        let store = lock_store(&state.store);
        let db = store
            .db()
            .ok_or_else(|| "banco nao esta aberto".to_string())?;
        db.exec("VACUUM").map_err(|e| format!("vacuum: {e}"))?;
    }
    let depois = std::fs::metadata(&caminho).map(|m| m.len()).unwrap_or(0);
    log(&format!("compact_db: {antes} -> {depois} bytes"));
    Ok(serde_json::json!({ "before": antes, "after": depois }))
}

fn copy_entry_inner(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    plain_text: bool,
) -> Result<(), String> {
    let (entry, image) = {
        let store = lock_store(&state.store);
        let entry = store
            .get(id)
            .cloned()
            .ok_or_else(|| "entry not found".to_string())?;
        let image = store.image_bytes(id);
        (entry, image)
    };

    capture::write_entry(&entry, image.as_deref(), plain_text)?;
    // Nossas próprias cópias não podem re-entrar no histórico. Guarda as
    // últimas 8 sequências: paste_many escreve várias seguidas e o watcher só
    // consome uma por polling — com um slot único, cópias intermediárias
    // eram capturadas como itens novos.
    {
        let seq = win32::sequence_number();
        let mut g = lock_ignores(&state.ignore_sequences);
        g.push(seq);
        let extra = g.len().saturating_sub(8);
        g.drain(..extra);
    }

    reusa_entrada(app, id);
    notify_changed(app);
    Ok(())
}

/// Entrada reusada vai para o topo (o Raycast faz o mesmo) — sem tocar no
/// conteúdo dela: fixação, OCR e metadados continuam.
fn reusa_entrada(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let mut store = lock_store(&state.store);
    if let Some(idx) = store.entries.iter().position(|e| e.id == id) {
        let mut e = store.entries.remove(idx);
        e.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(e.created_at);
        store.touch(&e);
        store.entries.insert(0, e);
    }
}

#[tauri::command]
fn copy_entry(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    plain_text: bool,
) -> Result<(), String> {
    copy_entry_inner(&app, &state, &id, plain_text)
}

/// Arquivos no disco de uma entrada, para **entregar como arquivo**
/// (`CF_HDROP`): tanto no arrasto para fora quanto na colagem de várias.
///
/// Ordem: arquivos do disco → texto que é caminho de arquivo → imagem/GIF que
/// só existe no banco cifrado (materializada uma vez em `<dados>/drag/`).
///
/// Entrada de vários arquivos (copiar três no Explorer cria **um** item com
/// três caminhos) devolve os três: entregar só o primeiro arrastava/colava um
/// pedaço do conteúdo sem avisar.
///
/// `Err` quando não há arquivo (texto, link, cor) e quando o caminho não serve
/// para o Windows entregar: de rede (`\\svr\...`, bug drag-rs #72) ou que sumiu
/// do disco (o alvo criaria "nada" e o usuário veria um gesto/colagem vazios).
fn arquivos_da_entrada(
    entry: &crate::store::Entry,
    imagem: Option<Vec<u8>>,
    dir: &std::path::Path,
    id: &str,
) -> Result<Vec<std::path::PathBuf>, String> {
    if !entry.paths.is_empty() {
        let mut prontos: Vec<std::path::PathBuf> = Vec::new();
        for p in entry.paths.iter().map(std::path::PathBuf::from) {
            // Caminho de rede ou que sumiu: erro, não "entrega o resto".
            caminho_de_arrasto(&p)?;
            prontos.push(p);
        }
        if !prontos.is_empty() {
            return Ok(prontos);
        }
    }
    if let Some(p) = caminho_do_texto(entry) {
        if p.is_file() {
            caminho_de_arrasto(&p)?;
            return Ok(vec![p]);
        }
    }
    let Some(imagem) = imagem else {
        return Err("this entry has no file".into());
    };
    let pasta = dir.join("drag");
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    // Nome barato antes de encodar: o hover chama isto a cada passada do mouse e
    // reencodar a imagem só para descobrir que o arquivo já existe é desperdício.
    let pronto = pasta.join(nome_materializado(entry, id));
    if pronto.is_file() {
        return Ok(vec![pronto]);
    }
    let payload = monta_payload(entry, Some(imagem), id)?;
    let Some(bytes) = b64_decode(&payload.data) else {
        return Err("could not prepare the file".into());
    };
    // **O mesmo** nome do caminho rápido: `payload.name` sozinho não é único (ver
    // `nome_materializado`), e escrever nele reintroduzia a colisão.
    let alvo = pronto;
    if !alvo.is_file() {
        std::fs::write(&alvo, &bytes).map_err(|e| e.to_string())?;
    }
    Ok(vec![alvo])
}

/// Cola várias entradas em sequência (a ordem do vetor é a ordem da colagem).
#[tauri::command]
fn paste_many(
    app: AppHandle,
    state: State<AppState>,
    ids: Vec<String>,
    plain_text: bool,
    keep_open: Option<bool>,
) -> Result<usize, String> {
    if ids.is_empty() {
        return Err("nothing selected".into());
    }
    let target = state.last_foreground.load(Ordering::SeqCst);
    let count = ids.len();
    log(&format!("paste_many n={count} target_hwnd={target}"));

    copy_entry_inner(&app, &state, &ids[0], plain_text)?;
    let _ = app.emit("clipboard:pasted", ());

    let keep_open = keep_open.unwrap_or_else(|| {
        app.state::<AppState>()
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keep_open_after_paste
    });
    if keep_open {
        std::thread::sleep(Duration::from_millis(45));
    } else if let Some(win) = app.get_webview_window("main") {
        // Esconde já (ShowWindow direto), como na colagem simples: o Windows
        // devolve o foco ao app anterior e a primeira colagem sai na hora.
        set_shown(false);
        win32::hide_now(main_hwnd(&app));
        let handle_win = win.clone();
        std::thread::spawn(move || {
            let _ = handle_win.hide();
        });
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let _trava = lock_paste();
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                let state = handle.state::<AppState>();
                if let Err(e) = copy_entry_inner(&handle, &state, id, plain_text) {
                    log(&format!("paste_many copy {id}: {e}"));
                    break;
                }
                std::thread::sleep(Duration::from_millis(90));
            }
            paste_with_retry(target);
            if i + 1 < count {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
        let _ = handle.emit("clipboard:multi-pasted", count);
    });
    Ok(count)
}

/// Cola **vários arquivos de uma vez**: uma escrita de `CF_HDROP` com todos os
/// caminhos e **uma** colagem, em vez de N colagens em sequência como o
/// `paste_many` faz com texto.
///
/// Usado quando a seleção é só arquivo/imagem (o frontend decide); seleção
/// misturada continua pelo caminho de texto.
#[tauri::command]
fn paste_files(
    app: AppHandle,
    state: State<AppState>,
    ids: Vec<String>,
    keep_open: Option<bool>,
) -> Result<usize, String> {
    if ids.is_empty() {
        return Err("nothing selected".into());
    }
    let target = state.last_foreground.load(Ordering::SeqCst);
    let mut caminhos: Vec<String> = Vec::new();
    for id in &ids {
        let (entry, imagem, dir) = {
            let store = lock_store(&state.store);
            let Some(e) = store.get(id).cloned() else {
                return Err("entry not found".into());
            };
            let img = if e.has_image {
                store.image_bytes(id)
            } else {
                None
            };
            (e, img, store.dir.clone())
        };
        match arquivos_da_entrada(&entry, imagem, &dir, id) {
            Ok(ps) => caminhos.extend(ps.iter().map(|p| p.to_string_lossy().to_string())),
            Err(e) => {
                // Nada de colar metade da seleção sem avisar.
                log(&format!("paste_files: {id} sem arquivo ({e})"));
                return Err(e);
            }
        }
    }
    let count = caminhos.len();
    log(&format!("paste_files n={count} target_hwnd={target}"));

    {
        use clipboard_win::formats;
        let _guard = clipboard_win::Clipboard::new_attempts(8).map_err(|e| e.to_string())?;
        formats::FileList
            .write_clipboard(&caminhos[..])
            .map_err(|e| e.to_string())?;
    }
    // Nossa própria escrita não pode re-entrar no histórico.
    {
        let seq = win32::sequence_number();
        let mut g = lock_ignores(&state.ignore_sequences);
        g.push(seq);
        let extra = g.len().saturating_sub(8);
        g.drain(..extra);
    }
    // Vão para o topo, mantendo a ordem da seleção.
    for id in ids.iter().rev() {
        reusa_entrada(&app, id);
    }
    notify_changed(&app);
    let _ = app.emit("clipboard:pasted", ());

    let keep_open = keep_open.unwrap_or_else(|| {
        app.state::<AppState>()
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keep_open_after_paste
    });
    if keep_open {
        std::thread::sleep(Duration::from_millis(45));
    } else if let Some(win) = app.get_webview_window("main") {
        // Mesmo caminho da colagem simples: esconde já e o foco volta sozinho.
        set_shown(false);
        win32::hide_now(main_hwnd(&app));
        let handle_win = win.clone();
        std::thread::spawn(move || {
            let _ = handle_win.hide();
        });
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let _trava = lock_paste();
        paste_with_retry(target);
        let _ = handle.emit("clipboard:multi-pasted", count);
    });
    Ok(count)
}

/// Bytes da imagem de uma entrada (o conteúdo fica no banco cifrado).
fn image_bytes_of(state: &AppState, id: &str) -> Result<(Vec<u8>, bool), String> {
    let store = lock_store(&state.store);
    let entry = store
        .get(id)
        .cloned()
        .ok_or_else(|| "entry not found".to_string())?;
    if !entry.has_image {
        return Err("this entry has no image".into());
    }
    let bytes = store
        .image_bytes(id)
        .ok_or_else(|| "image data is missing".to_string())?;
    let is_gif = entry.gif || bytes.starts_with(b"GIF8");
    Ok((bytes, is_gif))
}

fn encode_image(bytes: &[u8], is_gif: bool, path: &str, ext: &str) -> Result<(), String> {
    let out: Vec<u8> = if is_gif && ext == "gif" {
        bytes.to_vec()
    } else {
        let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        let mut buf: Vec<u8> = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        match ext {
            "jpg" => {
                let rgb = img.to_rgb8();
                image::codecs::jpeg::JpegEncoder::new_with_quality(cursor, 92)
                    .encode_image(&rgb)
                    .map_err(|e| e.to_string())?;
            }
            "webp" => {
                let rgba = img.to_rgba8();
                image::codecs::webp::WebPEncoder::new_lossless(cursor)
                    .encode(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| e.to_string())?;
            }
            "bmp" => {
                img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Bmp)
                    .map_err(|e| e.to_string())?;
            }
            "gif" => {
                img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Gif)
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
            }
        }
        buf
    };
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// "Save as…" para **qualquer** entrada (imagem, texto, link, arquivo), com o
/// diálogo do Windows já filtrado pela extensão escolhida.
#[tauri::command]
fn save_entry_as(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    format: String,
) -> Result<Option<String>, String> {
    let entry = {
        let store = lock_store(&state.store);
        store.get(&id).cloned()
    }
    .ok_or_else(|| "entry not found".to_string())?;
    let ext = normalize_ext(&format, &entry);
    if !entry.has_image && entry.text.is_none() && entry.paths.is_empty() {
        return Err("this entry has nothing to save".into());
    }
    let default_name = format!("{}.{ext}", file_stem(&entry, 1));

    state.dialog_open.store(true, Ordering::SeqCst);
    let _guard = DialogGuard(app.clone());
    let Some(path) = win32::save_dialog(&default_name, &ext, "Save as") else {
        return Ok(None);
    };
    let (bytes, is_gif) = {
        let store = lock_store(&state.store);
        let img = if entry.has_image {
            store.image_bytes(&id)
        } else {
            None
        };
        let gif = entry.gif || img.as_deref().is_some_and(|b| b.starts_with(b"GIF8"));
        (img, gif)
    };
    write_entry_as(
        &entry,
        bytes.as_deref(),
        is_gif,
        std::path::Path::new(&path),
        &ext,
    )?;
    log(&format!("save_entry_as {id} -> {path} ({ext})"));
    Ok(Some(path))
}

/// Salva a imagem da entrada como PNG/JPEG/WEBP/BMP/GIF usando o diálogo do Windows.
#[tauri::command]
fn save_image_as(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    format: String,
) -> Result<Option<String>, String> {
    let (bytes, is_gif) = image_bytes_of(&state, &id)?;
    let fmt = format.to_lowercase();
    let ext = match fmt.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "gif" => "gif",
        _ => "png",
    };
    let default_name = format!("Rayclip {}.{ext}", chrono_label());

    // Enquanto o diálogo nativo estiver aberto nada pode esconder a janela.
    state.dialog_open.store(true, Ordering::SeqCst);
    let _guard = DialogGuard(app.clone());
    let Some(path) = win32::save_dialog(&default_name, ext, "Save image as") else {
        return Ok(None); // usuário cancelou
    };
    encode_image(&bytes, is_gif, &path, ext)?;
    log(&format!("save_image_as {id} -> {path} ({ext})"));
    Ok(Some(path))
}

/// Mesma coisa sem diálogo (usado nos testes automatizados).
///
/// Escreve em **qualquer** caminho recebido, então só existe nas builds de
/// desenvolvimento: em release a porta fica fechada (a interface nunca chama;
/// quem escreve arquivo em release é sempre pelo diálogo do Windows).
#[tauri::command]
fn save_image_to(
    state: State<AppState>,
    id: String,
    path: String,
    format: String,
) -> Result<String, String> {
    if !cfg!(debug_assertions) {
        return Err("save_image_to is a development-only command".into());
    }
    let (bytes, is_gif) = image_bytes_of(&state, &id)?;
    let fmt = format.to_lowercase();
    let ext = match fmt.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "gif" => "gif",
        _ => "png",
    };
    encode_image(&bytes, is_gif, &path, ext)?;
    Ok(path)
}

/// Item da lista de "Save as…" com várias entradas selecionadas.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SaveItem {
    id: String,
    /// Extensão desejada: "png", "jpg", "txt", "lua"… (vazia = padrão do tipo)
    #[serde(default)]
    format: String,
}

fn normalize_ext(format: &str, entry: &crate::store::Entry) -> String {
    let raw: String = format
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_lowercase();
    if !raw.is_empty() {
        return raw;
    }
    entry_default_ext(entry).to_string()
}

fn entry_default_ext(entry: &crate::store::Entry) -> &'static str {
    if entry.has_image {
        return if entry.gif { "gif" } else { "png" };
    }
    match entry.kind.as_str() {
        "link" => "url",
        "email" => "txt",
        _ => "txt",
    }
}

/// Nome de arquivo para o arrasto. Diferente do "Save as…": aqui o destino é
/// uma pasta qualquer e o nome aparece no Explorer, então arquivos usam só o
/// nome do arquivo (nada de "C Users Lugia Downloads ...") e o resto reaproveita
/// a limpeza do `file_stem`.
fn drag_file_stem(entry: &crate::store::Entry) -> String {
    if let Some(first) = entry.paths.first() {
        if let Some(nome) = std::path::Path::new(first)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
        {
            let limpo: String = nome
                .chars()
                .map(|c| if c.is_control() || r#"/\:*?"<>|"#.contains(c) { ' ' } else { c })
                .collect();
            let nome = limpo.split_whitespace().collect::<Vec<_>>().join(" ");
            if !nome.is_empty() {
                return nome;
            }
        }
    }
    file_stem(entry, 1)
}

/// Nome do arquivo que materializa uma entrada em `<dados>/drag/`.
///
/// **O id entra no nome** porque o nome sozinho não é único: imagem copiada usa
/// `Rayclip image <data-hora até o segundo>`, então **duas imagens diferentes
/// copiadas no mesmo segundo geravam o mesmo nome**. A segunda encontrava o
/// arquivo da primeira já pronto no caminho rápido (`is_file()`) e devolvia o
/// caminho dela — era isso que fazia escolher duas imagens diferentes no
/// multi-select e colar **a mesma duas vezes**: um único `CF_HDROP` com dois
/// caminhos iguais. Os 8 primeiros caracteres alfanuméricos do id (uuid v4)
/// bastam para separar, e o nome continua legível.
fn nome_materializado(entry: &crate::store::Entry, id: &str) -> String {
    let curto: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    let stem = drag_file_stem(entry);
    let ext = normalize_ext("", entry);
    if curto.is_empty() {
        format!("{stem}.{ext}")
    } else {
        format!("{stem} {curto}.{ext}")
    }
}

/// Nome de arquivo seguro derivado da entrada.
fn file_stem(entry: &crate::store::Entry, index: usize) -> String {
    let raw = if entry.has_image {
        format!("Rayclip image {}", chrono_label())
    } else {
        entry.title.lines().next().unwrap_or("").trim().to_string()
    };
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut name = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    // `String::truncate` entra em panic quando 45 cai no meio de um
    // caractere UTF-8 (titulo em japones, emoji, acento composto) - e era
    // panic na thread de UI ao salvar. Recua ate um limite valido.
    if name.len() > 45 {
        let mut cut = 45;
        while !name.is_char_boundary(cut) {
            cut -= 1;
        }
        name.truncate(cut);
    }
    let name = name.trim_matches([' ', '.', '-']).to_string();
    if name.is_empty() {
        return format!("Rayclip entry {index}");
    }
    // Nomes de dispositivo reservados do Win32: "CON", "NUL", "COM1"...
    // recusados mesmo com extensao ("CON.txt"). Prefixo "_" resolve.
    let base = name
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
        "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
        "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&base.as_str()) {
        format!("_{name}")
    } else {
        name
    }
}

/// Caminho livre: "name.ext", "name 2.ext", "name 3.ext"…
fn unique_path(dir: &str, stem: &str, ext: &str) -> std::path::PathBuf {
    let mut candidate = std::path::Path::new(dir).join(format!("{stem}.{ext}"));
    let mut n = 2;
    while candidate.exists() && n < 1000 {
        candidate = std::path::Path::new(dir).join(format!("{stem} {n}.{ext}"));
        n += 1;
    }
    candidate
}

/// Grava uma entrada no destino com a extensão pedida.
fn write_entry_as(
    entry: &crate::store::Entry,
    image: Option<&[u8]>,
    is_gif: bool,
    path: &std::path::Path,
    ext: &str,
) -> Result<(), String> {
    if let Some(bytes) = image {
        if is_gif && ext == "gif" {
            // preserva a animação gravando os bytes originais
            return std::fs::write(path, bytes).map_err(|e| e.to_string());
        }
        return encode_image(bytes, is_gif, &path.to_string_lossy(), ext);
    }
    // Entrada de arquivo (PDF, vídeo, doc): copia os bytes originais. Antes só
    // copiava quando a extensão batia; com "Save as TXT" num PDF caía no ramo
    // de texto (que é `None` para arquivos) e gravava um arquivo de 0 bytes.
    if let Some(first) = entry.paths.first() {
        let p = std::path::Path::new(first);
        if p.is_file() {
            return std::fs::copy(p, path)
                .map(|_| ())
                .map_err(|e| e.to_string());
        }
    }
    let body = if ext == "html" {
        entry
            .html
            .clone()
            .or_else(|| entry.text.clone())
            .unwrap_or_default()
    } else if ext == "rtf" {
        entry
            .rtf
            .clone()
            .or_else(|| entry.text.clone())
            .unwrap_or_default()
    } else {
        entry
            .text
            .clone()
            .or_else(|| entry.ocr_text.clone())
            .unwrap_or_default()
    };
    std::fs::write(path, body).map_err(|e| e.to_string())
}

/// Grava as entradas na pasta pedida (sem diálogo). Devolve os caminhos.
fn write_items(state: &AppState, items: &[SaveItem], dir: &str) -> Result<Vec<String>, String> {
    let entries: Vec<(String, crate::store::Entry, Option<Vec<u8>>)> = {
        let store = lock_store(&state.store);
        items
            .iter()
            .filter_map(|i| {
                store.get(&i.id).map(|e| {
                    let img = if e.has_image {
                        store.image_bytes(&i.id)
                    } else {
                        None
                    };
                    (i.format.clone(), e.clone(), img)
                })
            })
            .collect()
    };
    if entries.is_empty() {
        return Err("selected entries no longer exist".into());
    }
    // O destino vem da interface (pasta escolhida no diálogo): só grava em
    // diretório que já existe, e nunca em cima de um arquivo.
    let destino = std::path::Path::new(dir);
    if !destino.is_dir() {
        return Err("the destination folder does not exist".into());
    }
    let mut written = Vec::new();
    for (idx, (format, entry, image)) in entries.iter().enumerate() {
        let ext = normalize_ext(format, entry);
        let stem = file_stem(entry, idx + 1);
        let path = unique_path(dir, &stem, &ext);
        let is_gif = entry.gif || image.as_deref().is_some_and(|b| b.starts_with(b"GIF8"));
        match write_entry_as(entry, image.as_deref(), is_gif, &path, &ext) {
            Ok(()) => {
                log(&format!("save_many {} -> {}", entry.id, path.display()));
                written.push(path.display().to_string());
            }
            Err(e) => log(&format!(
                "save_many {} falhou em {:?}: {e}",
                entry.id,
                path.display().to_string()
            )),
        }
    }
    if written.is_empty() {
        return Err("nothing could be saved".into());
    }
    Ok(written)
}

/// "Save as…" com **várias** entradas: escolhe uma pasta e grava uma lista de
/// arquivos, cada um no formato que o usuário escolheu na UI.
#[tauri::command]
fn save_many(
    app: AppHandle,
    state: State<AppState>,
    items: Vec<SaveItem>,
) -> Result<Option<Vec<String>>, String> {
    if items.is_empty() {
        return Err("nothing selected".into());
    }
    // Mantém a janela viva enquanto o seletor de pasta estiver aberto.
    state.dialog_open.store(true, Ordering::SeqCst);
    let _guard = DialogGuard(app.clone());
    let Some(dir) = win32::pick_folder("Choose a folder for the selected items") else {
        log("save_many: cancelado pelo usuário");
        return Ok(None);
    };
    let written = write_items(&state, &items, &dir)?;
    log(&format!("save_many n={} dir={dir}", written.len()));
    Ok(Some(written))
}

/// Mesmo salvamento sem diálogo (usado nos testes automatizados).
#[tauri::command]
fn save_many_to(
    state: State<AppState>,
    items: Vec<SaveItem>,
    dir: String,
) -> Result<Vec<String>, String> {
    if !cfg!(debug_assertions) {
        return Err("save_many_to is a development-only command".into());
    }
    let written = write_items(&state, &items, &dir)?;
    log(&format!("save_many_to n={} dir={dir}", written.len()));
    Ok(written)
}

fn chrono_label() -> String {
    // "2026-11-19 21-14-08" no fuso **local** do sistema.
    //
    // Antes isto convertia de UTC e o nome do arquivo saía adiantado pelo fuso:
    // numa máquina em UTC-3, uma imagem capturada às 21:12 virava
    // `Rayclip image 2026-11-25 00-12-26.png` — dia seguinte e três horas no
    // futuro em relação ao que a lista mostra. `GetLocalTime` já resolve horário
    // de verão, coisa que somar um `Bias` fixo não faz.
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let t = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}-{:02}-{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

#[tauri::command]
fn paste_entry(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    plain_text: bool,
    keep_open: Option<bool>,
) -> Result<(), String> {
    let target = state.last_foreground.load(Ordering::SeqCst);
    log(&format!("paste_entry id={id} target_hwnd={target}"));
    let t0 = std::time::Instant::now();
    copy_entry(app.clone(), state, id, plain_text)?;
    log(&format!("paste_entry: copiado em {:?}", t0.elapsed()));
    // Deixa a barrinha pintar o status verde antes de esconder a janela.
    let _ = app.emit("clipboard:pasted", ());
    let keep_open = keep_open.unwrap_or_else(|| {
        app.state::<AppState>()
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keep_open_after_paste
    });
    if !keep_open {
        // Esconde imediatamente: o Windows devolve o foco ao app anterior e a
        // thread de colagem espera esse instante (sem `sleep` fixo).
        if let Some(win) = app.get_webview_window("main") {
            hide_fast(&win, main_hwnd(&app));
        }
    }
    std::thread::spawn(move || {
        let _trava = lock_paste();
        if keep_open {
            // A janela continua aberta: dá um instante para o painel pintar.
            std::thread::sleep(Duration::from_millis(45));
        }
        paste_with_retry(target);
    });
    Ok(())
}

/// Resposta das mutações (fixar, renomear, editar, apagar).
///
/// Antes cada uma devolvia o histórico inteiro — 1,8 MB por clique. A interface
/// não usa mais esse payload (ela pede a página de novo, §8.44); aqui vai só o
/// que o aviso da tela precisa.
#[derive(serde::Serialize)]
struct Mudanca {
    /// Quantas entradas saíram (0 quando a ação não apagou nada).
    removed: usize,
    /// Quantas continuam no histórico.
    total: usize,
}

fn mudanca(state: &AppState, removed: usize) -> Mudanca {
    let total = lock_store(&state.store).entries.len();
    Mudanca { removed, total }
}

/// Mesma resposta, mas com o total que já está na mão **dentro** do lock.
///
/// Chamar `mudanca()` de dentro de `lock_store` trava tudo: o mutex do store não
/// é reentrante (aconteceu de verdade no primeiro teste do §8.46 — o app
/// congelou e o `delete_recent` nunca respondeu).
fn mudanca_com(removed: usize, total: usize) -> Mudanca {
    Mudanca { removed, total }
}

/// Renomeia uma entrada (título escolhido pelo usuário). O título só é
/// derivado do conteúdo na captura/importação, então o nome dado aqui fica.
#[tauri::command]
fn rename_entry(_app: AppHandle, state: State<AppState>, id: String, title: String) -> Mudanca {
    let clean: String = title.trim().chars().take(300).collect();
    {
        let mut store = lock_store(&state.store);
        let changed = store.entries.iter_mut().find(|e| e.id == id).map(|e| {
            e.title = clean.clone();
            e.clone()
        });
        if let Some(e) = changed {
            store.touch(&e);
            log(&format!(
                "rename_entry {} [{}] -> {:?}",
                e.id, e.kind, clean
            ));
        }
    }
    mudanca(&state, 0)
}

#[tauri::command]
fn toggle_pin(_app: AppHandle, state: State<AppState>, id: String) -> Mudanca {
    {
        let mut store = lock_store(&state.store);
        store.toggle_pin(&id);
    }
    mudanca(&state, 0)
}

/// Sobe (`up = true`) ou desce uma entrada dentro do grupo "Fixados".
#[tauri::command]
fn move_pinned(_app: AppHandle, state: State<AppState>, id: String, up: bool) -> Mudanca {
    {
        let mut store = lock_store(&state.store);
        let moved = store.move_pinned(&id, up);
        if !moved {
            log(&format!("move_pinned {id} ignorado (nao fixado ou no limite)"));
        }
    }
    mudanca(&state, 0)
}

/// Troca o **conteúdo** de uma entrada (o título só muda se ainda era derivado
/// do texto, mesma regra da captura). Usado por "Edit Content…".
#[tauri::command]
fn update_entry_content(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    text: String,
) -> Mudanca {
    {
        let mut store = lock_store(&state.store);
        let snap = match store.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                let old_title = e.title.clone();
                e.size_bytes = text.len() as u64;
                // O hash acompanha o conteúdo, senão recopiar o texto editado
                // criaria uma entrada paralela em vez de reusar esta.
                e.hash = crate::capture::hash_of(&["text", text.as_str()]);
                if e.title == old_title && (e.kind == "text" || e.kind == "link" || e.kind == "email" || e.kind == "color") {
                    e.title = crate::capture::one_line(&text);
                }
                e.text = Some(text.clone());
                Some(e.clone())
            }
            None => None,
        };
        if let Some(e) = snap {
            store.touch(&e);
            log(&format!("update_content {} ({} bytes)", e.id, e.size_bytes));
        }
    }
    notify_changed(&app);
    mudanca(&state, 0)
}

/// Escreve texto puro no clipboard (usado por "Copy Color as…" e afins).
#[tauri::command]
fn copy_text(state: State<AppState>, text: String) -> Result<(), String> {
    crate::capture::write_plain_text(&text)?;
    // Nossa própria cópia não volta como item novo do histórico.
    {
        let seq = win32::sequence_number();
        let mut g = lock_ignores(&state.ignore_sequences);
        g.push(seq);
        let extra = g.len().saturating_sub(8);
        g.drain(..extra);
    }
    Ok(())
}

/// Apaga as entradas **não fixadas** copiadas nos últimos `minutes` minutos
/// ("Delete Recent Entries…" do Raycast, que nunca toca nos fixados).
#[tauri::command]
fn delete_recent(_app: AppHandle, state: State<AppState>, minutes: u32) -> Mudanca {
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
        - (minutes as i64) * 60_000;
    {
        let mut store = lock_store(&state.store);
        let doomed: Vec<String> = store
            .entries
            .iter()
            .filter(|e| !e.pinned && e.created_at >= cutoff)
            .map(|e| e.id.clone())
            .collect();
        log(&format!("delete_recent {minutes}min -> {} entradas", doomed.len()));
        for id in &doomed {
            store.remove(id);
        }
        mudanca_com(doomed.len(), store.entries.len())
    }
}

#[tauri::command]
fn delete_entry(_app: AppHandle, state: State<AppState>, id: String) -> Mudanca {
    log(&format!("delete_entry id={id}"));
    {
        let mut store = lock_store(&state.store);
        let existia = store.entries.iter().any(|e| e.id == id);
        store.remove(&id);
        mudanca_com(if existia { 1 } else { 0 }, store.entries.len())
    }
}

/// Apaga várias entradas de uma vez (uma única travada do store e uma única
/// listagem de volta — apagar 200 itens um a um custava 200 payloads).
#[tauri::command]
fn delete_entries(_app: AppHandle, state: State<AppState>, ids: Vec<String>) -> Mudanca {
    log(&format!("delete_entries n={}", ids.len()));
    {
        let mut store = lock_store(&state.store);
        let antes = store.entries.len();
        for id in &ids {
            store.remove(id);
        }
        let saiu = antes - store.entries.len();
        mudanca_com(saiu, store.entries.len())
    }
}

#[tauri::command]
fn clear_entries(_app: AppHandle, state: State<AppState>, keep_pinned: bool) -> Mudanca {
    log(&format!("clear_entries keep_pinned={keep_pinned}"));
    {
        let mut store = lock_store(&state.store);
        let antes = store.entries.len();
        let kept: Vec<Entry> = store
            .entries
            .drain(..)
            .filter(|e| keep_pinned && e.pinned)
            .collect();
        store.entries = kept;
        store.save();
        let saiu = antes - store.entries.len();
        mudanca_com(saiu, store.entries.len())
    }
}

/// Texto integral de uma entrada (a lista manda só os primeiros 16 KB).
#[tauri::command]
fn entry_text(state: State<AppState>, id: String) -> Option<String> {
    let store = lock_store(&state.store);
    store
        .entries
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.text.clone())
}

/// Posição do ícone da bandeja na tela (para o teste automatizado clicar nele).
#[tauri::command]
fn tray_rect(app: AppHandle) -> Option<(i32, i32, i32, i32)> {
    let tray = app.tray_by_id("rayclip-tray")?;
    let r = tray.rect().ok()??;
    let p = r.position.to_physical::<i32>(1.0);
    let s = r.size.to_physical::<i32>(1.0);
    Some((p.x, p.y, s.width, s.height))
}

/// Porta de teste: chama o **mesmo** caminho que o ícone da bandeja usa.
#[tauri::command]
fn simular_bandeja(app: AppHandle) {
    log("teste: simular_bandeja");
    show_window(&app);
}

#[tauri::command]
fn hide_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        hide_fast(&win, main_hwnd(&app));
    }
}

#[tauri::command]
fn reveal_path(path: String) {
    use std::os::windows::process::CommandExt;
    // Só revela caminhos que existem: `explorer` com argumento inválido abre a
    // pasta Documentos e deixa uma janela extra na tela.
    if !std::path::Path::new(&path).exists() {
        log(&format!("reveal_path: nao existe: {path}"));
        return;
    }
    let _ = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&path)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn();
}

/// Abre link no navegador. Aceita **apenas** http/https/mailto: o conteúdo vem
/// da área de transferência (não confiável) e antes ia para `cmd /C start`, que
/// interpretava `&`, `|`, `%VAR%` etc. como comando.
/// Tipo de conteúdo de um arquivo de mídia pela extensão. `None` = não é
/// vídeo/áudio conhecido (a rota responde 415 em vez de servir bytes).
fn mime_de_midia(p: &std::path::Path) -> Option<&'static str> {
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    Some(match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "ogv" => "video/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "oga" => "audio/ogg",
        _ => return None,
    })
}

/// Resposta para `<video>`/`<audio>`: **um pedaço** do arquivo por requisição.
///
/// O elemento de mídia do Chromium pede `Range` para tocar, buscar posição e
/// saber a duração; responder o arquivo inteiro de uma vez faz um vídeo de
/// 300 MB virar 300 MB de memória do app. Sem `Range`, devolve o começo como
/// parcial (`206` + `Content-Range`) — é legal e informa o tamanho real, então
/// o player pede o resto.
fn resposta_midia(p: &std::path::Path, range: Option<&str>) -> tauri::http::Response<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    use tauri::http::{Response, StatusCode};

    const PEDACO: u64 = 8 * 1024 * 1024;

    let Some(mime) = mime_de_midia(p) else {
        return Response::builder()
            .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    };
    let Ok(meta) = std::fs::metadata(p) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    };
    let total = meta.len();
    if total == 0 {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }

    // "bytes=ini-fim" | "bytes=ini-" | "bytes=-tamanho"
    let mut ini = 0u64;
    let mut fim = total - 1;
    let mut parcial = false;
    if let Some(r) = range {
        if let Some(resto) = r.trim().strip_prefix("bytes=") {
            let mut partes = resto.splitn(2, '-');
            let a = partes.next().unwrap_or("").trim().to_string();
            let b = partes.next().unwrap_or("").trim().to_string();
            if a.is_empty() && !b.is_empty() {
                // sufixo: últimos N bytes
                let n: u64 = b.parse().unwrap_or(0);
                ini = total.saturating_sub(n);
            } else {
                ini = a.parse().unwrap_or(0);
                if !b.is_empty() {
                    fim = b.parse().unwrap_or(total - 1).min(total - 1);
                }
            }
            if ini > fim {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header("Content-Range", format!("bytes */{total}"))
                    .body(Vec::new())
                    .unwrap_or_else(|_| Response::new(Vec::new()));
            }
            parcial = true;
        }
    } else if total > PEDACO {
        // Sem Range num arquivo grande: entrega o primeiro pedaço como parcial.
        fim = PEDACO - 1;
        parcial = true;
    }

    let quer = fim - ini + 1;
    let mut buf = vec![0u8; quer as usize];
    let lidos = (|| -> Option<usize> {
        let mut f = std::fs::File::open(p).ok()?;
        f.seek(SeekFrom::Start(ini)).ok()?;
        let mut lido = 0usize;
        while lido < buf.len() {
            match f.read(&mut buf[lido..]) {
                Ok(0) => break,
                Ok(n) => lido += n,
                Err(_) => return None,
            }
        }
        Some(lido)
    })()
    .unwrap_or(0);
    buf.truncate(lidos);

    let mut b = Response::builder()
        .header("Content-Type", mime)
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "no-store");
    b = if parcial {
        b.status(StatusCode::PARTIAL_CONTENT)
            .header("Content-Range", format!("bytes {ini}-{fim}/{total}"))
    } else {
        b.status(StatusCode::OK)
    };
    b.body(buf).unwrap_or_else(|_| Response::new(Vec::new()))
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let u = url.trim();
    let low = u.to_ascii_lowercase();
    let ok =
        low.starts_with("http://") || low.starts_with("https://") || low.starts_with("mailto:");
    if !ok {
        log(&format!("open_url: esquema recusado: {u}"));
        return Err("Only http(s) and mailto links are opened".into());
    }
    if !win32::shell_open(u) {
        return Err("Could not open the link".into());
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
struct Bounds {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn bounds_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let dir = store::app_dir(&dir);
    Some(dir.join("window-v2.json"))
}

fn load_bounds(app: &AppHandle) -> Option<Bounds> {
    let path = bounds_file(app)?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_bounds(app: &AppHandle, b: Bounds) {
    let Some(path) = bounds_file(app) else { return };
    // Moved/Resized disparam a cada pixel do arrasto: nao gravar o JSON em
    // toda chamada. Guarda o ultimo valor e um thread grava a cada 600 ms.
    let mut g = BOUNDS_PENDING.lock().unwrap_or_else(|e| e.into_inner());
    *g = Some((path, b));
}

static BOUNDS_PENDING: std::sync::Mutex<Option<(std::path::PathBuf, Bounds)>> =
    std::sync::Mutex::new(None);

fn write_bounds_now() {
    let pending = {
        let mut g = BOUNDS_PENDING.lock().unwrap_or_else(|e| e.into_inner());
        g.take()
    };
    if let Some((path, b)) = pending {
        if let Ok(raw) = serde_json::to_string(&b) {
            let _ = std::fs::write(path, raw);
        }
    }
}

fn start_bounds_flusher() {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(600));
        write_bounds_now();
    });
}

/// Arredonda os cantos da janela como no Raycast (DWM, Windows 11).
fn round_corners(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        };
        if let Ok(hwnd) = win.hwnd() {
            let pref = DWMWCP_ROUND;
            unsafe {
                DwmSetWindowAttribute(
                    hwnd.0 as _,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    &pref as *const _ as *const core::ffi::c_void,
                    std::mem::size_of_val(&pref) as u32,
                );
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = win;
}

pub fn log(msg: &str) {
    // Build de release não escreve log por padrão. `RAYCLIP_LOG=1` liga **nesta
    // execução** (mesmo espírito do `RAYCLIP_CDP`): é o que permite diagnosticar
    // um problema que só aparece no build otimizado, sem mandar um binário de
    // depuração para quem está com o app instalado.
    if !cfg!(debug_assertions) && std::env::var_os("RAYCLIP_LOG").is_none() {
        return;
    }
    use std::io::Write;
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("rayclip.log"))
    {
        // Timestamp com milissegundos: é como medimos latência de hide/paste.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let ms = now.as_millis() % 1000;
        let secs = now.as_secs() % 86_400;
        let _ = writeln!(
            f,
            "{:02}:{:02}:{:02}.{:03} {msg}",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60,
            ms
        );
    }
}

fn show_window(app: &AppHandle) {
    let hwnd = win32::foreground_window();
    if let Some(state) = app.try_state::<AppState>() {
        // Only remember windows from other processes as paste target.
        let mut pid = 0u32;
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
            GetWindowThreadProcessId(hwnd as _, &mut pid);
        }
        if pid != win32::current_pid() && hwnd != 0 {
            state.last_foreground.store(hwnd, Ordering::SeqCst);
        }
        log(&format!(
            "show_window hwnd={hwnd} pid={pid} own={}",
            win32::current_pid()
        ));
    }
    if let Some(state) = app.try_state::<AppState>() {
        *lock_shown_at(&state.shown_at) = std::time::Instant::now();
    }
    set_shown(true);
    // Automaticamente ganhamos o foco ao aparecer; avisamos o Windows que
    // podemos devolvê-lo ao app anterior na colagem, sem passar pelo truque
    // lento do AttachThreadInput.
    win32::allow_foreground_any();
    if let Some(win) = app.get_webview_window("main") {
        // Medido com A/B: `win.show()` (abaixo) e um `ShowWindow` direto dão o
        // mesmo tempo até a janela aparecer (2 ms, 5 de 5). Fica o caminho
        // simples do Tauri — a versão "rápida" foi descartada por não ter ganho.
        // A política vem **antes** do `show()`: pedida depois, o primeiro quadro
        // já foi montado com o NC ligado e a legenda aparece por um instante.
        //
        // O **vidro** não é reafirmado aqui de propósito: medido, o painel já
        // nasce translúcido no primeiro quadro (luminância 62,5 sobre fundo
        // branco desde 0 ms) e reescrever `DWMWA_SYSTEMBACKDROP_TYPE` na abertura
        // custa **33 ms** de latência (2,1 -> 35,1 ms), porque o DWM reconstrói o
        // vidro e a chamada espera a composição.
        if let Ok(h) = win.hwnd() {
            win32::politica_dwm(h.0 as isize);
        }
        // `set_always_on_top` faz o tao reaplicar os estilos — e ele repõe
        // `WS_CAPTION`. Por isso ele vem **antes** do `show()`: assim o bit é
        // arrancado enquanto a janela ainda está invisível.
        let _ = win.set_always_on_top(true);
        if let Ok(h) = win.hwnd() {
            win32::tira_legenda(h.0 as isize);
        }
        let _ = win.show();
        if let Ok(h) = win.hwnd() {
            // `show()` e `set_always_on_top()` fazem o tao reaplicar os estilos
            // da janela, e `to_window_styles()` do tao repõe `WS_CAPTION` em
            // toda janela sem moldura. A subclasse já corta isso, mas reafirmar
            // aqui é barato (3 chamadas) e garante que nada pisca na abertura.
            let hr = win32::sem_moldura(h.0 as isize);
            if hr != 0 {
                log(&format!("moldura: DWM recusou o NC (hr=0x{hr:08x})"));
            }
            // Número medido em cada abertura: se a faixa não for (0, 0), é ali
            // que a barra clássica cabe.
            log(&format!("moldura: faixa na abertura = {:?}", win32::faixa_nc(h.0 as isize)));
            win32::force_foreground(h.0 as isize);
        }
        let _ = app.emit("clipboard:focus", ());
    }
}

/// Vigia de estado: devolve o que o Windows tira com o tempo (§8.78).
///
/// Relato do usuário: depois de horas com o modo translúcido ligado e navegando
/// pelos links, o painel **perde a transparência** e o **atalho de abrir/fechar
/// para de responder**, sem nada no histórico explicando. Os dois são estados
/// que o Windows pode tirar de nós:
///
/// - o DWM pode largar o `DWMWA_SYSTEMBACKDROP_TYPE` (troca de GPU, suspensão do
///   monitor, reinício do compositor) e nada volta a pedir — o painel fica sólido
///   para sempre;
/// - o registro do atalho global pode sumir (outro app pedindo a combinação,
///   reinício do shell) e ninguém percebe.
///
/// A cada 30 s, sem custo perceptível, os dois são conferidos e reaplicados **só
/// quando estão errados** — e cada correção fica no log com o nome do que voltou.
fn vigia_estado(app: AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        // 1) Atalhos globais.
        let guardados = ATALHOS_ATIVOS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some((mostrar, colar)) = guardados {
            let gs = app.global_shortcut();
            let perdidos: Vec<String> = [mostrar, colar]
                .into_iter()
                .filter(|c| !c.trim().is_empty() && !gs.is_registered(c.as_str()))
                .collect();
            if !perdidos.is_empty() {
                log(&format!("vigia: atalho(s) perdido(s) {perdidos:?} -> reaplicando"));
                let mut cfg = app
                    .try_state::<AppState>()
                    .map(|st| st.settings.lock().unwrap_or_else(|e| e.into_inner()).clone())
                    .unwrap_or_default();
                let avisos = apply_shortcuts(&app, &mut cfg);
                log(&format!("vigia: atalhos reaplicados ({avisos:?})"));
            }
        }
        // 2) Vidro do DWM.
        let quer_vidro = app
            .try_state::<AppState>()
            .map(|st| st.settings.lock().unwrap_or_else(|e| e.into_inner()).translucent)
            .unwrap_or(false)
            && sysinteg::windows_transparency();
        if quer_vidro {
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(h) = win.hwnd() {
                    // Consulta barata; a escrita (33 ms medidos) só quando o
                    // DWM de fato largou o vidro.
                    if win32::backdrop_tipo(h.0 as isize) == 1 {
                        log("vigia: vidro perdido -> reaplicando o acrilico");
                        let escuro = app
                            .try_state::<AppState>()
                            .map(|st| {
                                st.settings.lock().unwrap_or_else(|e| e.into_inner()).theme != "light"
                            })
                            .unwrap_or(true);
                        win32::reaplica_vidro(h.0 as isize, escuro);
                    }
                }
            }
        }
    });
}

fn toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = visible_now(app);
        let focused = win.is_focused().unwrap_or(false);
        if visible && focused {
            set_shown(false);
            let _ = win.hide();
        } else {
            show_window(app);
        }
    }
}

/// Vigia de foco: se a janela está visível mas o primeiro plano do Windows é
/// outro app, esconde. Cobre cliques fora que não geram evento de blur (janela
/// elevada, always-on-top, WebView2 perdendo a mensagem) e evita que a janela
/// fique "presa" na tela.
fn start_focus_watch(app: AppHandle) {
    std::thread::spawn(move || {
        // Duas leituras seguidas de "outro app em primeiro plano" antes de
        // esconder: um solavanco de foco (abrir menu, abrir diálogo, trocar de
        // painel, console que pisca) não pode fazer a janela desaparecer.
        let mut foreign_streak = 0u8;
        let mut last_self = std::time::Instant::now();
        loop {
            // 150 ms: barato (só GetForegroundWindow/GetCursorPos) e cobre o
            // caso "cursor saiu da janela depois de um Alt+Tab".
            std::thread::sleep(Duration::from_millis(150));
            let Some(win) = app.get_webview_window("main") else {
                continue;
            };
            if !win.is_visible().unwrap_or(false) {
                foreign_streak = 0;
                continue;
            }
            let (since, busy) = match app.try_state::<AppState>() {
                Some(s) => (
                    lock_shown_at(&s.shown_at).elapsed(),
                    s.dialog_open.load(Ordering::SeqCst),
                ),
                None => continue,
            };
            // Não esconde enquanto a janela acabou de aparecer (o foco ainda
            // está sendo transferido) nem com um diálogo aberto.
            if since < Duration::from_millis(900) || busy {
                foreign_streak = 0;
                continue;
            }
            if !win32::foreground_is_foreign() {
                foreign_streak = 0;
                last_self = std::time::Instant::now();
                continue;
            }
            // Console que pisca (reg.exe, taskkill, cmd) não conta como troca
            // de app: aparece e some em milissegundos.
            if win32::foreground_is_transient() {
                foreign_streak = 0;
                continue;
            }
            // Mouse em cima da janela: o usuário está mexendo nela, então um
            // blur aqui é acidente (menu, diálogo, console piscando).
            let hwnd = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
            if win32::cursor_over_window(hwnd) {
                foreign_streak = 0;
                continue;
            }
            // Acabamos de perder o foco: dá um instante para ele voltar.
            if last_self.elapsed() < Duration::from_millis(300) {
                foreign_streak = 0;
                continue;
            }
            foreign_streak = foreign_streak.saturating_add(1);
            if foreign_streak >= 2 {
                foreign_streak = 0;
                hide_fast(&win, hwnd);
                log(&format!("focus-watch: hide [{}]", win32::foreground_desc()));
            }
        }
    });
}

/// Decide e esconde **imediatamente** — chamado direto do callback do hook de
/// primeiro plano, sem thread intermediária. Só usa chamadas Win32 baratas
/// (GetCursorPos/GetWindowRect/class name) + dois atômicos.
fn try_hide_now(app: &AppHandle, fg_hwnd: isize) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if !visible_now(app) {
        return;
    }
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    // Diálogo nativo aberto ou janela recém-aberta: não é troca de app.
    if state.dialog_open.load(Ordering::SeqCst) {
        log("hook: ignora (dialogo aberto)");
        return;
    }
    if lock_shown_at(&state.shown_at).elapsed() < Duration::from_millis(150) {
        // O clique pode ter vindo junto com a abertura. Em vez de ignorar (o
        // que deixava o app na tela até o watchdog), confirma ao vivo em 160 ms.
        static PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !PENDING.swap(true, Ordering::SeqCst) {
            log("hook: pedido logo na abertura, confirmando em 160 ms");
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(160));
                hide_if_really_left(&app);
                PENDING.store(false, Ordering::SeqCst);
            });
        }
        return;
    }
    if !win32::hwnd_is_foreign(fg_hwnd) {
        log("hook: ignora (janela nossa)");
        return;
    }
    if win32::hwnd_is_transient(fg_hwnd) {
        // Console pode ser só um "flash" de processo auxiliar… ou o terminal
        // onde o usuário realmente está. Confirma em 140 ms: se continuar lá,
        // esconde.
        log(&format!("hook: console {fg_hwnd}, confirmando em 140 ms"));
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(140));
            if win32::foreground_hwnd() == fg_hwnd && win32::is_window_visible(fg_hwnd) {
                if let Some(win) = app.get_webview_window("main") {
                    hide_fast(&win, main_hwnd(&app));
                    log("hook: hide (console confirmado)");
                }
            }
        });
        return;
    }
    // Não olha mais o cursor: se o primeiro plano é outro app de verdade, o
    // usuário saiu — esconder na hora. (O cursor continuar sobre a janela só
    // acontece em Alt+Tab, e antes isso segurava o app na tela.)
    hide_fast(&win, main_hwnd(app));
    log(&format!(
        "foreground-hook: hide [{}]",
        win32::foreground_desc()
    ));
}

/// Hash de conteúdo (mesma função usada na captura), para deduplicar.
pub fn hash_parts(parts: &[&str]) -> String {
    capture::hash_of(parts)
}

/// Hash sobre bytes (imagens), sem o `from_utf8_lossy` que colapsava conteúdo.
pub fn hash_bytes(parts: &[&[u8]]) -> String {
    capture::hash_bytes(parts)
}

/// Heurística de link (usada também na importação do Raycast).
pub fn looks_like_link(s: &str) -> bool {
    capture::looks_like_link(s)
}

/// Esconde de verdade: `ShowWindow(SW_HIDE)` na hora e, em seguida, o
/// `hide()` do Tauri só para manter o estado interno dele em sincronia.
fn hide_fast(win: &tauri::WebviewWindow, hwnd: isize) {
    // Durante um arrasto para fora o foco vai para o Explorer: esconder aqui
    // mataria o arrasto no meio.
    if ARRASTANDO.load(Ordering::SeqCst) {
        log("hide ignorado: arrasto em andamento");
        return;
    }
    set_shown(false);
    win32::hide_now(hwnd);
    // `hide()` só serve para o estado interno do Tauri; ele despacha para a
    // thread principal (que pode estar ocupada), então roda fora do caminho.
    let win = win.clone();
    std::thread::spawn(move || {
        let _ = win.hide();
    });
}

/// A janela está visível? Pergunta ao Win32 em vez de ao Tauri — `is_visible()`
/// faz round-trip com a thread principal e trava quando ela está pintando.
fn visible_now(_app: &AppHandle) -> bool {
    SHOWN.load(Ordering::SeqCst)
}

fn main_hwnd(app: &AppHandle) -> isize {
    app.get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0)
}

/// Esconde **só se** o app realmente perdeu o foco. Lê o estado ao vivo
/// (primeiro plano, diálogo, visibilidade) em vez de confiar no evento que
/// pediu o hide — é o que permite confirmar pedidos que chegaram cedo demais.
fn hide_if_really_left(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if !visible_now(app) {
        return;
    }
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.dialog_open.load(Ordering::SeqCst) {
        return;
    }
    let fg = win32::foreground_hwnd();
    if !win32::hwnd_is_foreign(fg) {
        log("recheck: continua nosso, nao esconde");
        return;
    }
    hide_fast(&win, main_hwnd(app));
    log(&format!("recheck: hide [{}]", win32::foreground_desc()));
}

/// Esconde no instante do **clique fora** da janela (hook global de mouse).
/// Não depende do outro app processar a ativação: o app sai junto com o botão.
fn start_click_hook(app: AppHandle) {
    let (tx, rx) = std::sync::mpsc::channel::<isize>();
    let tx = std::sync::Mutex::new(tx);
    win32::on_mouse_down(move |target| {
        // Barato de propósito: nada de chamadas ao Tauri dentro do hook de
        // baixo nível (se demorar, o Windows descarta o hook).
        if let Ok(tx) = tx.lock() {
            let _ = tx.send(target);
        }
    });
    std::thread::spawn(move || {
        while let Ok(target) = rx.recv() {
            click_outside(&app, target);
        }
    });
}

fn click_outside(app: &AppHandle, target: isize) {
    if target == 0 {
        return;
    }
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.dialog_open.load(Ordering::SeqCst) {
        return; // diálogo nativo nosso aberto
    }
    // A janela raiz do ponto é a nossa? (Dentro do app o alvo do clique é um
    // filho do WebView2, que pode nem estar no nosso processo.)
    let ours = main_hwnd(app);
    let root = win32::root_of(target);
    if ours != 0 && (root == ours || target == ours) {
        return; // clicou na nossa janela
    }
    let target_pid = win32::pid_of_hwnd(target);
    // (o diagnóstico `hook diag: pt=(…)` da investigação do clique saiu: os
    // argumentos do `format!` chamavam `cursor_point`, `class_of_hwnd`,
    // `foreground_hwnd` e `window_rect` a cada clique **de qualquer janela**,
    // mesmo em release, onde `log` descarta a mensagem.)
    if target_pid == std::process::id() || win32::pid_of_hwnd(root) == std::process::id() {
        return; // menu / diálogo nativo nosso
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if !visible_now(app) {
        return;
    }
    let since = lock_shown_at(&state.shown_at).elapsed();
    let cls = win32::class_of_hwnd(target);
    // Barra de tarefas/bandeja: o 2º clique de um duplo-clique no ícone da
    // bandeja chega poucos ms depois de mostrar a janela — não pode esconder.
    let shell = cls == "Shell_TrayWnd" || cls == "Shell_SecondaryTrayWnd" || cls == "TrayNotifyWnd";
    if shell && since < Duration::from_millis(450) {
        log(&format!("click-hook: ignora bandeja cedo ({since:?})"));
        return;
    }
    hide_fast(&win, main_hwnd(app));
    log(&format!(
        "click-hook: hide (pid={target_pid} class={cls} desde_show={since:?})"
    ));
}

/// Caminho **rápido** para esconder: usa o hook de sistema
/// (`SetWinEventHook`/`EVENT_SYSTEM_FOREGROUND`), que avisa no instante em que
/// outro app vira primeiro plano. A decisão roda no próprio callback.
fn start_foreground_hook(app: AppHandle) {
    win32::on_foreground_change(move |fg| try_hide_now(&app, fg));
}

/// Quem colocou o conteúdo no clipboard: nome amigável + ícone do app.
///
/// Quando quem escreveu foi o próprio Rayclip (inclusive o processo do WebView2,
/// que é filho nosso), o "Source" vira **Rayclip** em vez de `msedgewebview2`.
fn source_of_clipboard(media_dir: &std::path::Path) -> (Option<String>, Option<String>) {
    let own = win32::clipboard_owner_full();
    let Some((pid, exe)) = own.or_else(win32::foreground_full) else {
        return (None, None);
    };
    let self_owned = win32::is_self_or_child(pid);
    log(&format!("source: {exe} (self={self_owned})"));
    let (name, icon_exe) = if self_owned {
        (
            "Rayclip".to_string(),
            std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| exe.clone()),
        )
    } else {
        (win32::friendly_source_name(&exe), exe)
    };
    let stem: String = std::path::Path::new(&icon_exe)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let out = media_dir.join(format!("src-{stem}.png"));
    let icon = thumbs::file_icon(std::path::Path::new(&icon_exe), &out, 32)
        .map(|p| p.to_string_lossy().to_string());
    (Some(name), icon)
}

/// Última sequência de clipboard já processada. `swap` garante que listener e
/// polling nunca processem a mesma cópia duas vezes.
static LAST_SEQ: AtomicU32 = AtomicU32::new(0);

/// Uma passada de captura: se a sequência do clipboard mudou, lê o conteúdo e
/// registra no histórico. Chamada pelo listener do Windows (instantâneo) e pelo
/// polling de segurança.
fn capture_cycle(app: &AppHandle) {
    let seq = win32::sequence_number();
    if LAST_SEQ.swap(seq, Ordering::SeqCst) == seq {
        return;
    }

    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return,
    };
    let was_ours = {
        let mut g = lock_ignores(&state.ignore_sequences);
        match g.iter().position(|s| *s == seq) {
            Some(i) => {
                g.remove(i);
                true
            }
            None => false,
        }
    };
    if was_ours {
        return;
    }
    let (ignore_pm, keep_sensitive) = {
        let cfg = lock_settings(&state.settings);
        (cfg.ignore_password_managers, cfg.capture_sensitive)
    };
    let Some(captured) = capture::read_current(ignore_pm, keep_sensitive) else {
        return;
    };
    let (source, source_icon) = {
        let media_dir = lock_store(&state.store).media_dir();
        source_of_clipboard(&media_dir)
    };
    let added = {
        let store = lock_store(&state.store);
        capture::build_entry(captured, &store, source, source_icon)
    };
    if let Some(built) = added {
        let capture::Built {
            entry,
            image,
            thumb,
        } = built;
        let is_image = entry.kind == "image";
        let new_id = entry.id.clone();
        let is_new = {
            let mut store = lock_store(&state.store);
            let dup = store.entries.iter().any(|e| e.hash == entry.hash);
            store.add(entry);
            match (image, thumb) {
                (Some(img), Some(th)) => store.put_image(&new_id, &img, Some(&th)),
                (Some(img), None) => store.put_image(&new_id, &img, None),
                (None, Some(th)) => store.put_thumb(&new_id, &th),
                (None, None) => {}
            }
            !dup
        };
        if is_new {
            if visible_now(app) {
                let _ = app.emit("clipboard:new", ());
            }
            if is_image {
                run_ocr_for(app.clone(), new_id.clone());
            }
            run_link_for(app.clone(), new_id, false);
        }
    }
}

fn start_watcher(app: AppHandle) {
    LAST_SEQ.store(win32::sequence_number(), Ordering::SeqCst);
    let (tx, rx) = std::sync::mpsc::channel::<()>();

    // 1) Listener do Windows (`AddClipboardFormatListener`): avisa no **instante**
    //    da cópia. Era o que faltava: com polling de 320 ms, duas cópias
    //    seguidas na mesma janela de tempo perdiam a primeira para sempre.
    let tx_listener = tx.clone();
    win32::on_clipboard_change(move || {
        let _ = tx_listener.send(());
    });

    // 2) Rede de segurança: se o listener não registrar (sistema antigo), o
    //    polling ainda pega a mudança — mais devagar, mas pega.
    let tx_poll = tx.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(700));
        let _ = tx_poll.send(());
    });

    // 3) Todo o trabalho pesado (ler clipboard, gerar imagem, gravar) acontece
    //    aqui, fora da thread da janela de mensagens do listener.
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            capture_cycle(&app);
        }
    });
}

/// Ícone na bandeja do Windows. Clique esquerdo mostra a janela; o menu
/// (clique direito) tem Open / Settings / Quit.
fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::with_id(app, "open", "Open Rayclip", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Rayclip", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &settings, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id("rayclip-tray")
        .tooltip("Rayclip · Clipboard History")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "settings" => {
                show_window(app);
                let _ = app.emit("rayclip:settings", ());
            }
            "quit" => {
                log("tray: quit");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                log("tray: clicado");
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    log("tray: criado");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Porta de depuração pedida por `RAYCLIP_CDP`, se houver.
///
/// Valores aceitos: `1` (porta 9222) ou o número da porta. Sem a variável,
/// nenhuma porta é aberta — é isso que permite o build de teste e o de release
/// serem o mesmo arquivo.
fn cdp_pedida() -> Option<u16> {
    let pedido = std::env::var("RAYCLIP_CDP").ok()?.trim().to_string();
    match pedido.as_str() {
        "" | "0" => None,
        "1" => Some(9222),
        outro => match outro.parse::<u16>() {
            Ok(p) if p > 1024 => Some(p),
            _ => {
                log(&format!("cdp: valor ignorado ({outro}); use 1 ou uma porta"));
                None
            }
        },
    }
}

/// Liga o DevTools **só nesta execução**, quando `RAYCLIP_CDP` existe.
///
/// Mexe na configuração **em memória**, antes de o `run` criar as janelas: o
/// `tauri.conf.json` fica sem `additionalBrowserArgs`, então o release não abre
/// porta nenhuma e não existe mais a obrigação de tirar/colocar a flag a cada
/// medição (a regra antiga do plano).
///
/// A variável de ambiente do WebView2 (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`)
/// **não** serve aqui: o wry sempre define os argumentos por conta própria e,
/// nesse caso, o WebView2 ignora a variável (medido: porta não abre).
///
/// Os argumentos logo abaixo são os mesmos que o wry usaria por padrão; passar
/// `additional_browser_args` **substitui** esse padrão, não acrescenta.
/// Argumentos de navegador em vigor nesta execução.
///
/// O WebView2 cria o ambiente **uma vez** por processo, com os `additional_browser_args`
/// da primeira janela. Criar outra janela com argumentos diferentes devolve
/// `HRESULT(0x8007139F)` (`ERROR_INVALID_STATE`) e a janela não nasce — é o
/// bug tauri#11144. Quem cria janela extra (a de metadados, `webmeta.rs`) tem de
/// repetir exatamente estes argumentos.
static ARGS_WEBVIEW2: Mutex<Option<String>> = Mutex::new(None);

pub fn args_webview2() -> Option<String> {
    ARGS_WEBVIEW2.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn aplicar_cdp(contexto: &mut tauri::Context) {
    let Some(porta) = cdp_pedida() else {
        return;
    };
    let args = format!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
         --autoplay-policy=no-user-gesture-required \
         --remote-debugging-address=127.0.0.1 --remote-debugging-port={porta}"
    );
    for janela in contexto.config_mut().app.windows.iter_mut() {
        janela.additional_browser_args = Some(args.clone());
    }
    *ARGS_WEBVIEW2.lock().unwrap_or_else(|e| e.into_inner()) = Some(args.clone());
    log(&format!("cdp: ligado em 127.0.0.1:{porta} (RAYCLIP_CDP)"));
}

pub fn run() {
    // Depuração por CDP sob demanda (ver `aplicar_cdp`).
    let mut contexto = tauri::generate_context!();
    aplicar_cdp(&mut contexto);

    // Atualização: o exe novo sobe com `--after-update <pid>` e **espera** o
    // antigo sair antes de qualquer outra coisa (dois processos com o SQLite
    // aberto ao mesmo tempo é o que estraga o histórico). Sem o argumento, é
    // abertura normal: limpa sobras de um update interrompido.
    match update::pid_substituido() {
        Some(pid) => update::concluir_atualizacao(pid),
        None => update::limpar_inicio(),
    }

    // Rede de segurança: release **sempre** elevado. O manifesto já pede
    // `requireAdministrator` (o processo nem sobe sem elevação), mas se algum
    // build sair sem manifesto e a tarefa sem-UAC existir, reabre por ela e sai
    // — nunca rodar rebaixado, que é o que quebra colagem em janela elevada.
    if !cfg!(debug_assertions)
        && !sysinteg::is_elevated()
        && std::env::var("RAYCLIP_SEM_ELEVACAO").is_err()
        && sysinteg::run_launch_task()
    {
        return;
    }
    // `--open` faz o app abrir já na tela. Sem a flag ele nasce **na bandeja**,
    // que é o certo quando o Windows o inicia sozinho: ninguém quer a janela
    // pulando na frente no login. A flag existe para testar (e para quem quiser
    // abrir pelo atalho com a janela à mostra).
    // Argumentos de instalação: o gancho `POSTINSTALL`/`PREUNINSTALL` do NSIS
    // chama `rayclip.exe --iniciar-com-windows` (ou `--remover-do-inicio`) para
    // mexer no logon usando o **próprio app** — a tarefa agendada elevada é
    // criada do jeito que o resto do código espera. Faz o trabalho e sai sem
    // abrir janela nem bandeja.
    if std::env::args().any(|a| a == "--iniciar-com-windows" || a == "--remover-do-inicio") {
        let ligar = std::env::args().any(|a| a == "--iniciar-com-windows");
        let mut cfg = settings::Settings::load_sem_app();
        cfg.launch_at_login = ligar;
        match cfg.save_sem_app() {
            Ok(()) => log(&format!("instalador: iniciar_com_windows={ligar} gravado")),
            Err(e) => log(&format!("instalador: falha ao gravar ajuste: {e}")),
        }
        match sysinteg::set_startup(ligar) {
            Ok(()) => log(&format!("instalador: logon ajustado ({ligar})")),
            Err(e) => log(&format!("instalador: falha no logon: {e}")),
        }
        return;
    }

    let abrir_no_inicio = std::env::args().any(|a| a == "--open");
    log(&format!("inicio: abrir_no_inicio={abrir_no_inicio}"));

    if win32::already_running() {
        // Já existe uma instância: pede a ela que mostre a janela (duplo clique
        // no atalho, menu Iniciar, "Executar"…).
        win32::signal_activate();
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Arrasto para **fora** do app por OLE (`CF_HDROP` com o caminho): é o
        // único caminho sem teto de tamanho. O `DownloadURL` do Chromium leva os
        // bytes e para de funcionar entre 3 e 25 MB.
        .plugin(tauri_plugin_drag::init())
        // Imagens saem do banco cifrado por aqui: `http://rayclip.localhost/img/<id>`
        // (Windows não aceita esquema próprio sem esse ".localhost").
        .register_uri_scheme_protocol("rayclip", |ctx, request| {
            use tauri::http::{Response, StatusCode};
            let path = request.uri().path().trim_start_matches('/').to_string();
            let mut parts = path.splitn(2, '/');
            let kind = parts.next().unwrap_or("");
            let id = parts.next().unwrap_or("").trim_end_matches(".png");
            // Ícone do app de origem (`media/src-<app>.png`): vem por aqui em vez
            // do asset protocol do Tauri, que daria à interface acesso a
            // qualquer arquivo do disco. O nome é validado e só esta pasta é
            // servida.
            // Janela escondida devolvendo as URLs de mídia da página
            // (ver `webmeta.rs`): a página é de outro domínio, então a volta
            // vem por navegação para cá, não por IPC.
            if kind == "webmeta" {
                webmeta::receber(id);
                return Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Vec::new())
                    .unwrap_or_else(|_| Response::new(Vec::new()));
            }
            if kind == "src" {
                // O `id` chega sem o `.png` (o handler tira o sufixo de todas as
                // rotas), então o nome do arquivo é remontado aqui.
                let nome: String = id
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                let arquivo = format!("{nome}.png");
                let valido = nome.starts_with("src-") && nome.len() > 4;
                let media = {
                    let state = ctx.app_handle().state::<AppState>();
                    let store = lock_store(&state.store);
                    store.media_dir()
                };
                let bytes = if valido {
                    std::fs::read(media.join(&arquivo)).ok()
                } else {
                    None
                };
                return match bytes {
                    Some(b) => Response::builder()
                        .header("Content-Type", "image/png")
                        .header("Cache-Control", "public, max-age=31536000, immutable")
                        .body(b)
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                    None => Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Vec::new())
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                };
            }
            // Capa (og:image) guardada em `media/covers/<hash>`.
            if kind == "cover" {
                let key: String = id
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit())
                    .collect();
                let media = {
                    let state = ctx.app_handle().state::<AppState>();
                    let store = lock_store(&state.store);
                    store.media_dir()
                };
                let bytes = std::fs::read(media.join("covers").join(&key)).ok();
                let mime = bytes
                    .as_deref()
                    .map(|b| {
                        if b.starts_with(&[0x89, b'P', b'N', b'G']) {
                            "image/png"
                        } else if b.starts_with(&[0xFF, 0xD8]) {
                            "image/jpeg"
                        } else if b.starts_with(b"GIF8") {
                            "image/gif"
                        } else if linkmeta::capa_e_video(b) {
                            // Capa que é o loop mp4/webm de uma página de GIF.
                            if b.len() > 12 && &b[8..12] == b"WEBM" {
                                "video/webm"
                            } else {
                                "video/mp4"
                            }
                        } else if b.starts_with(b"RIFF") {
                            "image/webp"
                        } else {
                            "image/x-icon"
                        }
                    })
                    .unwrap_or("image/png");
                return match bytes {
                    Some(b) => Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "public, max-age=31536000, immutable")
                        .body(b)
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                    None => Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Vec::new())
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                };
            }
            // Vídeo/áudio do histórico: serve o arquivo **do disco** com Range.
            // O caminho é lido do banco a partir do id (a URL nunca carrega
            // caminho), e só sai daqui o que é vídeo/áudio conhecido.
            if kind == "video" || kind == "audio" {
                let caminho = {
                    let state = ctx.app_handle().state::<AppState>();
                    let store = lock_store(&state.store);
                    store
                        .get(id)
                        .and_then(|e| e.paths.first().map(std::path::PathBuf::from))
                };
                let range = request
                    .headers()
                    .get("range")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.to_string());
                return match caminho {
                    Some(p) => resposta_midia(&p, range.as_deref()),
                    None => Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Vec::new())
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                };
            }
            // Favicon por host: arquivo em `media/fav/<host>` (não é conteúdo
            // do clipboard, é ícone derivado — igual ao ícone do app de origem).
            if kind == "fav" {
                let host: String = id
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
                    .collect();
                let media = {
                    let state = ctx.app_handle().state::<AppState>();
                    let store = lock_store(&state.store);
                    store.media_dir()
                };
                let path = media.join("fav").join(&host);
                let mut bytes = std::fs::read(&path).ok();
                if bytes.is_none() && !host.is_empty() {
                    // Cache apagado (ou entrada importada): baixa uma vez, aqui,
                    // fora de qualquer lock — assim o ícone se conserta sozinho
                    // em vez de virar imagem quebrada.
                    let _ = linkmeta::favicon_url(&media, &format!("https://{host}/"));
                    bytes = std::fs::read(&path).ok();
                }
                let mime = bytes
                    .as_deref()
                    .map(|b| {
                        if b.starts_with(&[0x89, b'P', b'N', b'G']) {
                            "image/png"
                        } else if b.starts_with(&[0xFF, 0xD8]) {
                            "image/jpeg"
                        } else if b.starts_with(b"GIF8") {
                            "image/gif"
                        } else if b.starts_with(b"RIFF") {
                            "image/webp"
                        } else if b.starts_with(b"<svg") || b.starts_with(b"<?xml") {
                            "image/svg+xml"
                        } else {
                            "image/x-icon"
                        }
                    })
                    .unwrap_or("image/x-icon");
                return match bytes {
                    Some(b) => Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "public, max-age=31536000, immutable")
                        .body(b)
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                    None => Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Vec::new())
                        .unwrap_or_else(|_| Response::new(Vec::new())),
                };
            }
            let bytes = {
                let state = ctx.app_handle().state::<AppState>();
                let (cached, full) = {
                    let store = lock_store(&state.store);
                    match kind {
                        "thumb" => match store.thumb_bytes(id) {
                            Some(t) => (Some(t), None),
                            // Sem miniatura guardada: gera a partir da imagem.
                            None => (None, store.image_bytes(id)),
                        },
                        _ => (store.image_bytes(id), None),
                    }
                };
                match (cached, full) {
                    (Some(b), _) => Some(b),
                    (None, Some(src)) => {
                        // Fora do lock: decodificar/redimensionar leva centenas
                        // de ms em imagens grandes.
                        let th = thumbs::thumb_from_bytes(&src, 64);
                        if let Some(t) = th.as_ref() {
                            let mut store = lock_store(&state.store);
                            store.put_thumb(id, t);
                        }
                        th
                    }
                    (None, None) => None,
                }
            };
            match bytes {
                Some(bytes) => {
                    let mime = if kind == "thumb" {
                        "image/png"
                    } else if bytes.starts_with(b"GIF8") {
                        "image/gif"
                    } else if bytes.starts_with(b"RIFF") {
                        "image/webp"
                    } else if bytes.starts_with(&[0xFF, 0xD8]) {
                        "image/jpeg"
                    } else {
                        "image/png"
                    };
                    // A URL carrega `?v=<created_at>`: cache longo é seguro e
                    // evita re-decriptar o mesmo blob a cada rolagem.
                    Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "public, max-age=31536000, immutable")
                        .body(bytes)
                        .unwrap_or_else(|_| Response::new(Vec::new()))
                }
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap_or_else(|_| Response::new(Vec::new())),
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_entries,
            drag_payload,
            simular_bandeja,
            tray_rect,
        drag_files,
            drag_start,
            drag_end,
            search_entries,
            heartbeat,
            compact_db,
            ocr_pending,
            link_pending,
            link_web_meta,
            link_entry,
            thumb_entry,
            ocr_entry,
            ocr_disponivel,
            copy_ocr_text,
            copy_entry,
            paste_entry,
            paste_many,
            paste_files,
            save_image_as,
            save_entry_as,
            save_image_to,
            save_many,
            save_many_to,
            toggle_pin,
            move_pinned,
            update_entry_content,
            copy_text,
            delete_recent,
            rename_entry,
            delete_entry,
            delete_entries,
            clear_entries,
            import_raycast,
            raycast_import_count,
            hide_window,
            entry_text,
            reveal_path,
            open_url,
            update_check,
            update_install,
            get_settings,
            save_settings,
            restart_explorer,
            set_windows_transparency,
            system_status
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let dir = store::app_dir(&data_dir);
            let cfg = settings::Settings::load(app.handle());
            app.manage(AppState {
                store: Mutex::new(Store::load(dir)),
                last_foreground: AtomicIsize::new(0),
                ignore_sequences: Mutex::new(Vec::new()),
                shown_at: Mutex::new(std::time::Instant::now()),
                settings: Mutex::new(cfg.clone()),
                dialog_open: std::sync::atomic::AtomicBool::new(false),
            });
            {
                let state = app.state::<AppState>();
                let mut store = lock_store(&state.store);
                store.set_max_entries(cfg.max_entries);
                store.purge_older_than(cfg.retention_days);
            }

            // O app roda **elevado por padrão** (manifesto `requireAdministrator`
            // no release): é o que permite colar em janelas elevadas e mexer em
            // Win+V/autostart sem bloqueio. Aqui só registramos a tarefa que
            // reabre o app já elevado, sem prompt do UAC.
            let elevado = sysinteg::is_elevated();
            let tarefa = elevado && sysinteg::ensure_launch_task();
            log(&format!(
                "elevacao: rodando elevado={} tarefa_sem_uac={}",
                elevado, tarefa
            ));

            // Em thread: decodifica as imagens antigas em data URL e grava em
            // arquivo, sem atrasar a primeira abertura da janela.
            {
                let app = app.handle().clone();
                std::thread::spawn(move || migra_links_para_arquivos(&app));
            }

            if let Some(win) = app.get_webview_window("main") {
                let restored = load_bounds(app.handle())
                    .filter(|b| b.w >= 520.0 && b.h >= 340.0 && b.x > -2000.0 && b.y > -2000.0);
                if let Some(b) = restored {
                    let _ = win.set_size(tauri::LogicalSize::new(b.w, b.h));
                    let _ = win.set_position(tauri::LogicalPosition::new(b.x, b.y));
                } else {
                    let _ = win.set_size(tauri::LogicalSize::new(900.0, 570.0));
                    if let Some(monitor) = win.current_monitor().ok().flatten() {
                        let scale = monitor.scale_factor();
                        let size = monitor.size().to_logical::<f64>(scale);
                        let x = (size.width - 900.0) / 2.0;
                        let y = (size.height - 570.0) * 0.18;
                        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                    }
                }
                round_corners(&win);
                if let Ok(h) = win.hwnd() {
                    // Sem barra de título nativa em **nenhuma** máquina: num PC
                    // de outro usuário os três botões do Windows apareceram
                    // mesmo com `decorations: false`.
                    let hr = win32::sem_moldura(h.0 as isize);
                    log(&format!("moldura: subclasse instalada, dwm=0x{hr:08x}"));
                    // Mica (translúcido) já no primeiro desenho, sem "flash".
                    let blur = cfg.translucent && sysinteg::windows_transparency();
                    win32::apply_backdrop(
                        h.0 as isize,
                        if blur { 2 } else { 1 },
                        cfg.theme != "light",
                    );
                }
                // Lido de novo aqui (em vez de capturar de `run()`): a closure
                // do `setup` pode sobreviver à função, e um `bool` copiado evita
                // ter que transformar tudo em `move`.
                let abrir_no_inicio = std::env::args().any(|a| a == "--open");
                if abrir_no_inicio {
                    // Mesmo caminho do atalho: marca o estado de visibilidade e
                    // pede o foco, do jeito que o resto do app espera.
                    let _ = win.show();
                    let _ = win.set_focus();
                    set_shown(true);
                } else {
                    log("inicio: janela na bandeja (use --open para abrir na tela)");
                }

                // Vigia do renderizador (§8.43): sem batida de heartbeat por
                // 15 s com a janela na tela, recarrega a página em vez de
                // deixar um retângulo branco para sempre.
                ULTIMO_BATIMENTO.store(agora_ms(), Ordering::SeqCst);

                // Aquece o cache da busca (dobra dos campos): sem isso a
                // primeira tecla digitada paga ~200 ms dobrando 1.683 entradas.
                // Roda em thread própria e em prioridade abaixo do normal.
                {
                    let app_aquece = app.handle().clone();
                    std::thread::spawn(move || {
                        if let Some(st) = app_aquece.try_state::<AppState>() {
                            let copia = {
                                let store = lock_store(&st.store);
                                store.entries.clone()
                            };
                            let t0 = std::time::Instant::now();
                            search::aquecer(&copia);
                            log(&format!(
                                "busca: cache aquecido ({} entradas) em {} ms",
                                copia.len(),
                                t0.elapsed().as_millis()
                            ));
                        }
                    });
                }
                vigia_estado(app.handle().clone());
                {
                    let app_vigia = app.handle().clone();
                    std::thread::spawn(move || {
                        let nascimento = agora_ms();
                        loop {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        if !SHOWN.load(Ordering::SeqCst) {
                            continue;
                        }
                        let agora = agora_ms();
                        let ultimo = ULTIMO_BATIMENTO.load(Ordering::SeqCst);
                        // 20 s de carência: o WebView2 pode levar segundos para
                        // começar a rodar JS numa máquina carregada.
                        let idade = agora.saturating_sub(nascimento);
                        if idade < 20_000 || agora.saturating_sub(ultimo) < 15_000 {
                            continue;
                        }
                        let parado = agora.saturating_sub(ultimo);
                        let tentativa = TENTATIVAS_RELOAD.load(Ordering::SeqCst);
                        // Duas tentativas de `reload()` cobrem o caso normal
                        // (renderer morto). Se nem isso voltar, o ambiente
                        // inteiro do WebView2 morreu — `reload()` não recria o
                        // processo do navegador, então a saída é reiniciar o
                        // app (uma vez, para não virar laço).
                        if tentativa >= 2 {
                            log(&format!(
                                "renderer nao voltou apos {tentativa} reloads ({parado} ms) -> reiniciando o app"
                            ));
                            let exe = std::env::current_exe().ok();
                            if let Some(exe) = exe {
                                let _ = std::process::Command::new(exe)
                                    .arg("--open")
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn();
                            }
                            std::process::exit(0);
                        }
                        log(&format!(
                            "renderer morto: sem heartbeat ha {parado} ms -> reload ({})",
                            tentativa + 1
                        ));
                        TENTATIVAS_RELOAD.fetch_add(1, Ordering::SeqCst);
                        if let Some(w) = app_vigia.get_webview_window("main") {
                            match w.reload() {
                                Ok(()) => {}
                                Err(e) => log(&format!("reload falhou: {e}")),
                            }
                        }
                        ULTIMO_BATIMENTO.store(agora_ms(), Ordering::SeqCst);
                        }
                    });
                }
            }

            let mut cfg_atalhos = cfg.clone();
            let avisos_atalho = apply_shortcuts(app.handle(), &mut cfg_atalhos);
            log(&format!("atalhos: {} aviso(s)", avisos_atalho.len()));
            let _ = sysinteg::set_replace_win_v(cfg.replace_win_v);
            let _ = sysinteg::set_startup(cfg.launch_at_login);

            start_watcher(app.handle().clone());

            // Verificação silenciosa de atualização: 15 s depois de abrir e
            // depois a cada 6 h. Não baixa nada sozinha — só avisa a interface
            // ("update:available"); instalar é decisão do usuário.
            if cfg.auto_update_check {
                let h = app.handle().clone();
                std::thread::spawn(move || {
                    let mut espera = Duration::from_secs(15);
                    loop {
                        std::thread::sleep(espera);
                        espera = Duration::from_secs(6 * 3600);
                        if lock_settings(&h.state::<AppState>().settings).offline_mode {
                            continue;
                        }
                        match update::check(false) {
                            Ok(Some(info)) => {
                                log(&format!("update: v{} disponivel", info.version));
                                let _ = h.emit("update:available", info);
                            }
                            Ok(None) => {}
                            Err(e) => log(&format!("update: checagem falhou ({e})")),
                        }
                    }
                });
            }
            start_focus_watch(app.handle().clone());
            ocr::cleanup_temp();
            start_bounds_flusher();
            start_foreground_hook(app.handle().clone());
            start_click_hook(app.handle().clone());

            // Duplo clique no atalho / "Executar" com o app já aberto: mostra.
            {
                let h = app.handle().clone();
                win32::start_activate_watch(move || show_window(&h));
            }

            // Ícone na bandeja (tray): clique esquerdo mostra, menu tem
            // abrir / configurações / sair.
            setup_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
                set_shown(false);
            }
            tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
                if window.is_visible().unwrap_or(false) {
                    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) {
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let b = Bounds {
                            x: pos.x as f64 / scale,
                            y: pos.y as f64 / scale,
                            w: size.width as f64 / scale,
                            h: size.height as f64 / scale,
                        };
                        save_bounds(window.app_handle(), b);
                    }
                }
            }
            tauri::WindowEvent::Focused(false) => {
                // O WebView2 às vezes "rouba" o foco da janela tao (menus,
                // dropdowns, páginas internas), então o único juiz é o primeiro
                // plano real do Windows: se ainda é um processo nosso (inclusive
                // o diálogo "Salvar como"), não esconde.
                let w = window.clone();
                let app = window.app_handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(220));
                    if let Some(state) = app.try_state::<AppState>() {
                        let since = lock_shown_at(&state.shown_at).elapsed();
                        let settle = Duration::from_millis(700);
                        if since < settle {
                            std::thread::sleep(settle - since);
                        }
                    }
                    if !w.is_visible().unwrap_or(false) {
                        return;
                    }
                    if let Some(state) = app.try_state::<AppState>() {
                        if state.dialog_open.load(Ordering::SeqCst) {
                            return;
                        }
                    }
                    // Confirma com uma segunda leitura antes de esconder.
                    if !win32::foreground_is_foreign() {
                        return;
                    }
                    // Mouse ainda sobre a janela = interação em andamento.
                    let hwnd = w.hwnd().map(|h| h.0 as isize).unwrap_or(0);
                    if win32::cursor_over_window(hwnd) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(260));
                    if w.is_visible().unwrap_or(false)
                        && win32::foreground_is_foreign()
                        && !win32::cursor_over_window(hwnd)
                    {
                        // `hide_fast` (e não `w.hide()` sozinho) para manter o
                        // estado SHOWN em sincronia com a janela real. O evento
                        // entrega um `Window` genérico; `hide_fast` precisa do
                        // `WebviewWindow`.
                        if let Some(wv) = app.get_webview_window("main") {
                            hide_fast(&wv, hwnd);
                        }
                        log(&format!("blur: hide [{}]", win32::foreground_desc()));
                    }
                });
            }
            _ => {}
        })
        .run(contexto)
        .expect("error while running rayclip");
}

#[cfg(test)]
mod arrasto {
    use super::*;

    /// O arrasto do sistema leva **caminho**, não bytes: arquivo local e
    /// existente tem que sair inteiro, sem teto de tamanho.
    #[test]
    fn arquivo_local_existente_vira_caminho() {
        let tmp = std::env::temp_dir().join("rayclip-teste-arrasto.txt");
        std::fs::write(&tmp, b"conteudo").unwrap();
        let saida = caminho_de_arrasto(&tmp).unwrap();
        assert!(saida.ends_with("rayclip-teste-arrasto.txt"));
        let _ = std::fs::remove_file(&tmp);
    }

    /// Arquivo que sumiu do disco: o arrasto não pode começar apontando para o
    /// vazio (o Explorer criaria nada e o usuário veria um gesto sem resultado).
    #[test]
    fn arquivo_inexistente_e_recusado() {
        let tmp = std::env::temp_dir().join("rayclip-nao-existe-9999.bin");
        let _ = std::fs::remove_file(&tmp);
        assert!(caminho_de_arrasto(&tmp).is_err());
    }

    /// Caminho de rede (`\\servidor\...) derruba o plugin de arrasto.
    #[test]
    fn caminho_de_rede_e_recusado() {
        let p = std::path::Path::new(r"\\servidor\pasta\arquivo.zip");
        let erro = caminho_de_arrasto(p).unwrap_err();
        assert!(erro.contains("network"), "erro inesperado: {erro}");
    }

    /// Diretório não é arquivo: `CF_HDROP` de pasta sai diferente no Explorer e
    /// não é o que a lista oferece.
    #[test]
    fn diretorio_e_recusado() {
        assert!(caminho_de_arrasto(&std::env::temp_dir()).is_err());
    }

    /// Entrada de texto (sem arquivo nenhum) não pode virar "arquivo": é o que
    /// separa a colagem de N arquivos da colagem de texto.
    #[test]
    fn texto_sem_arquivo_nao_vira_caminho() {
        let e = entry_vazio("text", "apenas um texto");
        let r = arquivos_da_entrada(&e, None, &std::env::temp_dir(), "t1");
        assert!(r.is_err(), "texto puro não tem caminho");
    }

    /// Entrada com `paths` entrega o arquivo, sem ler byte nenhum.
    #[test]
    fn caminho_da_entrada_vem_de_paths() {
        let tmp = std::env::temp_dir().join("rayclip-colar-1.bin");
        std::fs::write(&tmp, b"x").unwrap();
        let mut e = entry_vazio("file", "arquivo");
        e.paths = vec![tmp.to_string_lossy().to_string()];
        let p = arquivos_da_entrada(&e, None, &std::env::temp_dir(), "t2").unwrap();
        assert_eq!(p, vec![tmp.clone()]);
        let _ = std::fs::remove_file(&tmp);
    }

    /// Um item do histórico pode ser **vários** arquivos (copiar três no
    /// Explorer). Ele tem que entregar os três: antes só o primeiro saía e o
    /// resto desaparecia sem aviso.
    #[test]
    fn entrada_de_varios_arquivos_entrega_todos() {
        let a = std::env::temp_dir().join("rayclip-multi-a.txt");
        let b = std::env::temp_dir().join("rayclip-multi-b.txt");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let mut e = entry_vazio("files", "2 files");
        e.paths = vec![
            a.to_string_lossy().to_string(),
            b.to_string_lossy().to_string(),
        ];
        let ps = arquivos_da_entrada(&e, None, &std::env::temp_dir(), "t4").unwrap();
        assert_eq!(ps, vec![a.clone(), b.clone()]);
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    /// Arquivo apagado do disco: erro, e não um caminho morto na colagem.
    #[test]
    fn caminho_apagado_e_erro() {
        let mut e = entry_vazio("file", "fantasma");
        e.paths = vec![std::env::temp_dir()
            .join("rayclip-apagado-9999.bin")
            .to_string_lossy()
            .to_string()];
        assert!(arquivos_da_entrada(&e, None, &std::env::temp_dir(), "t3").is_err());
    }

    /// Mime por extensão: é o que decide se a rota serve ou responde 415.
    #[test]
    fn mime_de_midia_conhece_video_e_audio() {
        use std::path::Path;
        assert_eq!(mime_de_midia(Path::new("a.MP4")), Some("video/mp4"));
        assert_eq!(mime_de_midia(Path::new("b.webm")), Some("video/webm"));
        assert_eq!(mime_de_midia(Path::new("c.m4a")), Some("audio/mp4"));
        assert_eq!(mime_de_midia(Path::new("d.txt")), None);
        assert_eq!(mime_de_midia(Path::new("sem_extensao")), None);
    }

    fn arquivo_de_teste(nome: &str, tamanho: usize) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(nome);
        let bytes: Vec<u8> = (0..tamanho).map(|i| (i % 251) as u8).collect();
        std::fs::write(&p, &bytes).unwrap();
        p
    }

    /// `Range: bytes=10-19` devolve 206 com o pedaço e o tamanho total — é o
    /// que o `<video>` usa para tocar e buscar posição.
    #[test]
    fn range_de_video_devolve_pedaco() {
        let f = arquivo_de_teste("rayclip-range.mp4", 100);
        let r = resposta_midia(&f, Some("bytes=10-19"));
        assert_eq!(r.status(), 206);
        assert_eq!(r.headers().get("content-range").unwrap(), "bytes 10-19/100");
        assert_eq!(r.body().len(), 10);
        assert_eq!(r.body()[0], 10);
        let _ = std::fs::remove_file(&f);
    }

    /// Sufixo (`bytes=-5`) = últimos N bytes.
    #[test]
    fn range_de_sufixo_pega_o_final() {
        let f = arquivo_de_teste("rayclip-sufixo.mp4", 100);
        let r = resposta_midia(&f, Some("bytes=-5"));
        assert_eq!(r.status(), 206);
        assert_eq!(r.headers().get("content-range").unwrap(), "bytes 95-99/100");
        assert_eq!(r.body().len(), 5);
        let _ = std::fs::remove_file(&f);
    }

    /// Sem Range: 200 com o arquivo inteiro (arquivo pequeno).
    #[test]
    fn sem_range_devolve_arquivo_inteiro() {
        let f = arquivo_de_teste("rayclip-inteiro.mp4", 100);
        let r = resposta_midia(&f, None);
        assert_eq!(r.status(), 200);
        assert_eq!(r.body().len(), 100);
        assert_eq!(r.headers().get("accept-ranges").unwrap(), "bytes");
        let _ = std::fs::remove_file(&f);
    }

    /// Range impossível responde 416 com o tamanho real (o player recua).
    #[test]
    fn range_invalido_responde_416() {
        let f = arquivo_de_teste("rayclip-416.mp4", 100);
        let r = resposta_midia(&f, Some("bytes=500-600"));
        assert_eq!(r.status(), 416);
        assert_eq!(r.headers().get("content-range").unwrap(), "bytes */100");
        let _ = std::fs::remove_file(&f);
    }

    /// Extensão que não é mídia: 415, para a rota não servir qualquer arquivo.
    #[test]
    fn arquivo_que_nao_e_midia_responde_415() {
        let f = arquivo_de_teste("rayclip-texto.txt", 10);
        assert_eq!(resposta_midia(&f, None).status(), 415);
        let _ = std::fs::remove_file(&f);
    }

    /// Entry mínimo para os testes (o resto dos campos tem `default`).
    /// Duas imagens diferentes escolhidas juntas colavam a **mesma** duas vezes.
    ///
    /// O nome do arquivo materializado era `Rayclip image <data-hora até o
    /// segundo>`: duas imagens copiadas no mesmo segundo davam o mesmo nome, e a
    /// segunda reusava o arquivo da primeira (`is_file()`). Aqui são duas
    /// entradas com esse nome idêntico e ids diferentes — precisam virar dois
    /// arquivos, cada um com os seus bytes.
    /// Versão determinística: mesmo nome-base (mesmo segundo), ids diferentes.
    #[test]
    fn nome_materializado_separa_pelo_id() {
        let mut a = entry_vazio("image", "");
        a.has_image = true;
        let n1 = nome_materializado(&a, "11111111-2222-3333-4444-555555555555");
        let n2 = nome_materializado(&a, "99999999-8888-7777-6666-555555555555");
        assert_ne!(n1, n2, "dois ids precisam dar dois nomes: {n1}");
        assert!(n1.contains("11111111"), "nome sem o id: {n1}");
        assert!(n1.ends_with(".png"), "extensao errada: {n1}");
    }

    #[test]
    fn duas_imagens_no_mesmo_segundo_nao_compartilham_arquivo() {
        let dir = std::env::temp_dir().join("rayclip-teste-drag-unicos");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut a = entry_vazio("image", "");
        a.has_image = true;
        let mut b = entry_vazio("image", "");
        b.has_image = true;

        let png_a: &[u8] = b"\x89PNG\r\n\x1a\nIMAGEM-A";
        let png_b: &[u8] = b"\x89PNG\r\n\x1a\nIMAGEM-B";
        let pa = arquivos_da_entrada(
            &a,
            Some(png_a.to_vec()),
            &dir,
            "11111111-2222-3333-4444-555555555555",
        )
        .unwrap();
        let pb = arquivos_da_entrada(
            &b,
            Some(png_b.to_vec()),
            &dir,
            "99999999-8888-7777-6666-555555555555",
        )
        .unwrap();

        assert_ne!(
            pa[0], pb[0],
            "duas imagens diferentes viraram o mesmo arquivo: {:?}",
            pa[0]
        );
        assert_eq!(std::fs::read(&pa[0]).unwrap(), png_a);
        assert_eq!(std::fs::read(&pb[0]).unwrap(), png_b);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn entry_vazio(kind: &str, texto: &str) -> crate::store::Entry {
        crate::store::Entry {
            id: "teste".into(),
            kind: kind.into(),
            title: texto.into(),
            text: Some(texto.into()),
            ..Default::default()
        }
    }
}
