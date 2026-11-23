p = 'src-tauri/src/lib.rs'
s = open(p, encoding='utf-8').read()


def rep(old, new, count=1):
    global s
    assert old in s, 'NAO ACHOU: ' + old[:90]
    s = s.replace(old, new, count)


# ---- system_status: estado da cifra ---------------------------------------
rep('''        "winTransparency": sysinteg::windows_transparency(),''',
'''        "winTransparency": sysinteg::windows_transparency(),
        "encrypted": encrypted_db,
        "dbPath": db_path,''')

rep('''fn system_status() -> serde_json::Value {''',
'''fn system_status(state: State<AppState>) -> serde_json::Value {
    let (encrypted_db, db_path) = {
        let store = state.store.lock().unwrap();
        (
            store.encrypted,
            store
                .dir
                .join(store::DB_FILE)
                .to_string_lossy()
                .to_string(),
        )
    };''')

# ---- comandos de importa��ao -------------------------------------------------
rep('''#[tauri::command]
fn list_entries(state: State<AppState>) -> Vec<Entry> {''',
'''/// Quantos itens existem no histórico do Raycast (para o botão do Settings).
#[tauri::command]
fn raycast_import_count() -> usize {
    import::default_dir().map(|d| import::count(&d)).unwrap_or(0)
}

/// Importa o histórico do Raycast para o banco cifrado do Rayclip.
#[tauri::command]
fn import_raycast(app: AppHandle, state: State<AppState>, path: Option<String>) -> Result<usize, String> {
    let dir = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => import::default_dir().ok_or_else(|| {
            "Raycast history not found (%LOCALAPPDATA%\\\\Raycast\\\\clipboard)".to_string()
        })?,
    };
    if !dir.is_dir() {
        return Err("folder not found".into());
    }
    let handle = app.clone();
    let total = {
        let mut store = state.store.lock().unwrap();
        if !store.encrypted {
            return Err("encrypted database is not available".into());
        }
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
    let _ = app.emit("clipboard:changed", ());
    Ok(total)
}

#[tauri::command]
fn list_entries(state: State<AppState>) -> Vec<Entry> {''')

# ---- protocolo de imagem --------------------------------------------------
rep('''    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())''',
'''    tauri::Builder::default()
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
                let store = state.store.lock().unwrap();
                match kind {
                    "img" => store.image_bytes(id),
                    "thumb" => store.thumb_bytes(id),
                    _ => None,
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
                    Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "no-store")
                        .body(bytes)
                        .unwrap_or_else(|_| Response::new(Vec::new()))
                }
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap_or_else(|_| Response::new(Vec::new())),
            }
        })''')

rep('''            clear_entries,
            hide_window,''','''            clear_entries,
            import_raycast,
            raycast_import_count,
            hide_window,''')
open(p, 'w', encoding='utf-8').write(s)
print('lib parte 3 ok')
