#![windows_subsystem = "windows"]

mod capture;
mod clipimg;
mod import;
mod key;
mod linkmeta;
mod ocr;
mod settings;
mod sqlite;
mod store;
mod sysinteg;
mod thumbs;
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
    pub ignore_sequence: AtomicU32,
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

/// (Re)registra os atalhos globais a partir das configurações.
fn apply_shortcuts(app: &AppHandle, cfg: &settings::Settings) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let handle = app.clone();
    gs.on_shortcuts(
        [cfg.show_hotkey.as_str(), cfg.paste_hotkey.as_str()],
        move |_a, _s, event| {
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                toggle_window(&handle);
            }
        },
    )
    .map_err(|e| format!("{e}"))
}

/// Aplica tudo que depende das configurações (atalhos, limite, retenção, Windows).
fn apply_settings(app: &AppHandle, cfg: &settings::Settings) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Err(e) = apply_shortcuts(app, cfg) {
        warnings.push(format!("shortcut: {e}"));
    }
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state.store);
        store.set_max_entries(cfg.max_entries);
        store.purge_older_than(cfg.retention_days);
    }
    if let Err(e) = sysinteg::set_replace_win_v(cfg.replace_win_v) {
        warnings.push(format!("win+v: {e}"));
    }
    if let Err(e) = sysinteg::set_startup(cfg.launch_at_login) {
        warnings.push(format!("startup: {e}"));
    }
    if let Some(win) = app.get_webview_window("main") {
        if let Ok(h) = win.hwnd() {
            win32::apply_backdrop(
                h.0 as isize,
                if cfg.translucent && sysinteg::windows_transparency() {
                    2
                } else {
                    1
                },
            );
        }
    }
    warnings
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
    let cfg = settings.sanitized();
    cfg.save(&app)?;
    *lock_settings(&state.settings) = cfg.clone();
    let warnings = apply_settings(&app, &cfg);
    log(&format!("settings saved ({} avisos)", warnings.len()));
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
            win32::apply_backdrop(h.0 as isize, if translucent && enable { 2 } else { 1 });
        }
    }
    Ok(sysinteg::windows_transparency())
}

/// Estado real do Windows: o Win+V nativo está desativado?
#[tauri::command]
fn system_status(state: State<AppState>) -> serde_json::Value {
    let (encrypted_db, db_path) = {
        let store = lock_store(&state.store);
        (
            store.encrypted,
            store.dir.join(store::DB_FILE).to_string_lossy().to_string(),
        )
    };
    serde_json::json!({
        "winVDisabled": sysinteg::replace_win_v_active(),
        "startupTask": sysinteg::startup_task_active(),
        "elevated": sysinteg::is_elevated(),
        "winTransparency": sysinteg::windows_transparency(),
        "encrypted": encrypted_db,
        "dbPath": db_path,
        "exe": std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
    })
}

/// Lock tolerante a **envenenamento**: se outra thread sofrer panic segurando o
/// mutex, `librar()` normal faria todo comando seguinte em pânico — o app inteiro
/// morreria. Aqui recuperamos o conteúdo e seguimos.
/// Quanto de texto vai para a lista (busca + prévia). O resto é buscado sob
/// demanda por `entry_text`, no painel de detalhes.
const LIST_TEXT_MAX: usize = 16 * 1024;

/// Corta uma string em `max` **bytes** sem quebrar caractere UTF-8.
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

fn lock_store(m: &Mutex<Store>) -> std::sync::MutexGuard<'_, Store> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock_settings(m: &Mutex<settings::Settings>) -> std::sync::MutexGuard<'_, settings::Settings> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock_shown_at(m: &Mutex<std::time::Instant>) -> std::sync::MutexGuard<'_, std::time::Instant> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A janela está na tela? Rastreamos nós mesmos: `IsWindowVisible` retorna
/// `true` desde a criação (o Tauri mapeia a janela mesmo com `visible: false`),
/// o que furava qualquer decisão de "mostrar/esconder".
static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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
            hash: e.hash.clone(),
        })
        .collect();
    list.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.created_at.cmp(&a.created_at))
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
        let Some(_slot) = acquire_job(20_000) else {
            return;
        };
        ocr_one(&app, &id);
    });
}

/// Indexa uma imagem (quem chama já segura uma vaga do pool).
fn ocr_one(app: &AppHandle, id: &str) -> bool {
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
fn run_link_for(app: AppHandle, id: String) {
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
        log(&format!("link: buscando {url}"));
        let Some(meta) = linkmeta::fetch(&url) else {
            log(&format!("link: sem metadados para {url}"));
            return;
        };
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

/// Quantos links ainda não têm metadados (indexa os antigos ao abrir).
#[tauri::command]
fn link_pending(app: AppHandle, state: State<AppState>) -> usize {
    let ids: Vec<String> = {
        let store = lock_store(&state.store);
        store
            .entries
            .iter()
            .filter(|e| e.kind == "link" && e.link.is_none())
            .take(12)
            .map(|e| e.id.clone())
            .collect()
    };
    let n = ids.len();
    for id in ids {
        run_link_for(app.clone(), id);
    }
    n
}

#[tauri::command]
fn link_entry(app: AppHandle, state: State<AppState>, id: String) -> bool {
    let ok = {
        let store = lock_store(&state.store);
        store.get(&id).map(|e| e.kind == "link").unwrap_or(false)
    };
    if ok {
        run_link_for(app, id);
    }
    ok
}

#[tauri::command]
fn ocr_pending(app: AppHandle, state: State<AppState>) -> usize {
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
    state
        .ignore_sequence
        .store(win32::sequence_number(), Ordering::SeqCst);

    // Keep history ordered like Raycast: re-used items move to the top.
    {
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
    notify_changed(app);
    Ok(())
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
            .unwrap()
            .keep_open_after_paste
    });
    if keep_open {
        std::thread::sleep(Duration::from_millis(120));
    } else {
        std::thread::sleep(Duration::from_millis(220));
        if let Some(win) = app.get_webview_window("main") {
            set_shown(false);
            let _ = win.hide();
        }
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                let state = handle.state::<AppState>();
                if let Err(e) = copy_entry_inner(&handle, &state, id, plain_text) {
                    log(&format!("paste_many copy {id}: {e}"));
                    break;
                }
                std::thread::sleep(Duration::from_millis(110));
            }
            win32::paste_into(target);
            if i + 1 < count {
                std::thread::sleep(Duration::from_millis(340));
            }
        }
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
#[tauri::command]
fn save_image_to(
    state: State<AppState>,
    id: String,
    path: String,
    format: String,
) -> Result<String, String> {
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
    name.truncate(45);
    let name = name.trim_matches([' ', '.', '-']).to_string();
    if name.is_empty() {
        format!("Rayclip entry {index}")
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
    // Entrada de arquivo (PDF, vídeo, doc): copia os bytes originais.
    if let Some(first) = entry.paths.first() {
        let p = std::path::Path::new(first);
        if p.is_file()
            && ext.eq_ignore_ascii_case(p.extension().and_then(|e| e.to_str()).unwrap_or(""))
        {
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
    let written = write_items(&state, &items, &dir)?;
    log(&format!("save_many_to n={} dir={dir}", written.len()));
    Ok(written)
}

fn chrono_label() -> String {
    // "2026-11-19 21-14-08" sem dependências de data
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // conversão civil (UTC) — suficiente para nome de arquivo único
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let mut z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    z = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * z + 2) / 153;
    let d = z - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}-{m:02}-{s:02}")
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
            .unwrap()
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
        if keep_open {
            // A janela continua aberta: dá um instante para o painel pintar.
            std::thread::sleep(Duration::from_millis(90));
        }
        win32::paste_into(target);
    });
    Ok(())
}

/// Renomeia uma entrada (título escolhido pelo usuário). O título só é
/// derivado do conteúdo na captura/importação, então o nome dado aqui fica.
#[tauri::command]
fn rename_entry(_app: AppHandle, state: State<AppState>, id: String, title: String) -> Vec<Entry> {
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
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}

#[tauri::command]
fn toggle_pin(_app: AppHandle, state: State<AppState>, id: String) -> Vec<Entry> {
    {
        let mut store = lock_store(&state.store);
        let changed = store.entries.iter_mut().find(|e| e.id == id).map(|e| {
            e.pinned = !e.pinned;
            e.clone()
        });
        if let Some(e) = changed {
            store.touch(&e);
        }
    }
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}

#[tauri::command]
fn delete_entry(_app: AppHandle, state: State<AppState>, id: String) -> Vec<Entry> {
    log(&format!("delete_entry id={id}"));
    {
        let mut store = lock_store(&state.store);
        store.remove(&id);
    }
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}

#[tauri::command]
fn clear_entries(_app: AppHandle, state: State<AppState>, keep_pinned: bool) -> Vec<Entry> {
    log(&format!("clear_entries keep_pinned={keep_pinned}"));
    {
        let mut store = lock_store(&state.store);
        let kept: Vec<Entry> = store
            .entries
            .drain(..)
            .filter(|e| keep_pinned && e.pinned)
            .collect();
        store.entries = kept;
        store.save();
    }
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
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
    if let Some(path) = bounds_file(app) {
        if let Ok(raw) = serde_json::to_string(&b) {
            let _ = std::fs::write(path, raw);
        }
    }
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
    if !cfg!(debug_assertions) {
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
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_always_on_top(true);
        if let Ok(h) = win.hwnd() {
            win32::force_foreground(h.0 as isize);
        }
        let _ = app.emit("clipboard:focus", ());
    }
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

/// Heurística de link (usada também na importação do Raycast).
pub fn looks_like_link(s: &str) -> bool {
    capture::looks_like_link(s)
}

/// Esconde de verdade: `ShowWindow(SW_HIDE)` na hora e, em seguida, o
/// `hide()` do Tauri só para manter o estado interno dele em sincronia.
fn hide_fast(win: &tauri::WebviewWindow, hwnd: isize) {
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

fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut seen = win32::sequence_number();
        loop {
            std::thread::sleep(Duration::from_millis(320));
            let seq = win32::sequence_number();
            if seq == seen {
                continue;
            }
            seen = seq;

            let state = match app.try_state::<AppState>() {
                Some(s) => s,
                None => continue,
            };
            if state.ignore_sequence.swap(0, Ordering::SeqCst) == seq {
                continue;
            }
            let ignore_pm = { lock_settings(&state.settings).ignore_password_managers };
            let Some(captured) = capture::read_current(ignore_pm) else {
                continue;
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
                    if visible_now(&app) {
                        let _ = app.emit("clipboard:new", ());
                    }
                    if is_image {
                        run_ocr_for(app.clone(), new_id.clone());
                    }
                    run_link_for(app.clone(), new_id);
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
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

pub fn run() {
    if win32::already_running() {
        // Já existe uma instância: pede a ela que mostre a janela (duplo clique
        // no atalho, menu Iniciar, "Executar"…).
        win32::signal_activate();
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Imagens saem do banco cifrado por aqui: `http://rayclip.localhost/img/<id>`
        // (Windows não aceita esquema próprio sem esse ".localhost").
        .register_uri_scheme_protocol("rayclip", |ctx, request| {
            use tauri::http::{Response, StatusCode};
            let path = request.uri().path().trim_start_matches('/').to_string();
            let mut parts = path.splitn(2, '/');
            let kind = parts.next().unwrap_or("");
            let id = parts.next().unwrap_or("").trim_end_matches(".png");
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
            ocr_pending,
            link_pending,
            link_entry,
            thumb_entry,
            ocr_entry,
            copy_ocr_text,
            copy_entry,
            paste_entry,
            paste_many,
            save_image_as,
            save_entry_as,
            save_image_to,
            save_many,
            save_many_to,
            toggle_pin,
            rename_entry,
            delete_entry,
            clear_entries,
            import_raycast,
            raycast_import_count,
            hide_window,
            entry_text,
            reveal_path,
            open_url,
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
                ignore_sequence: AtomicU32::new(0),
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
                    // Mica (translúcido) já no primeiro desenho, sem "flash".
                    let blur = cfg.translucent && sysinteg::windows_transparency();
                    win32::apply_backdrop(h.0 as isize, if blur { 2 } else { 1 });
                }
                let _ = win.show();
                let _ = win.set_focus();
            }

            match apply_shortcuts(app.handle(), &cfg) {
                Ok(()) => log("atalhos registrados"),
                Err(e) => log(&format!("shortcut registration: {e}")),
            }
            let _ = sysinteg::set_replace_win_v(cfg.replace_win_v);
            let _ = sysinteg::set_startup(cfg.launch_at_login);

            start_watcher(app.handle().clone());
            start_focus_watch(app.handle().clone());
            ocr::cleanup_temp();
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
                        let _ = w.hide();
                        log(&format!("blur: hide [{}]", win32::foreground_desc()));
                    }
                });
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running rayclip");
}
