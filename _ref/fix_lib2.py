p = 'src-tauri/src/lib.rs'
s = open(p, encoding='utf-8').read()


def rep(old, new, count=1):
    global s
    assert old in s, 'NAO ACHOU: ' + old[:90]
    s = s.replace(old, new, count)


# ---- OCR de bytes ----------------------------------------------------------
rep('''        let path = {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            store.get(&id).and_then(|e| e.image_path.clone())
        };
        let Some(path) = path else { return };
        let Some(text) = ocr::image_text(std::path::Path::new(&path)) else {
            log(&format!("ocr: no text in {id}"));
            return;
        };''',
'''        let bytes = {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            let entry = store.get(&id)?;
            if !entry.has_image {
                return;
            }
            store.image_bytes(&id)?
        };
        // Descarta quando não houver texto: nada muda no histórico.
        let Some(text) = ocr::image_text_bytes(&bytes) else {
            log(&format!("ocr: no text in {id}"));
            return;
        };''')

rep('''        {
            let state = app.state::<AppState>();
            let mut store = state.store.lock().unwrap();
            if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                e.ocr_text = Some(text);
            }
            store.save();
        }''',
'''        {
            let state = app.state::<AppState>();
            let mut store = state.store.lock().unwrap();
            if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                e.ocr_text = Some(text);
                let e = e.clone();
                store.touch(&e);
            }
        }''')

# ---- miniatura real de arquivo (thumb no banco) ---------------------------
rep('''        let st = app.state::<AppState>();
        let (path, out) = {
            let store = st.store.lock().unwrap();
            let Some(e) = store.get(&id) else { return };
            if e.thumb.is_some() {
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
            let out = store
                .images_dir()
                .join(format!("thumb-{}.png", linkmeta::short_hash(&p)));
            (p, out)
        };
        let Some(thumb) = thumbs::file_thumb(std::path::Path::new(&path), &out, 512) else {
            log(&format!("thumb: sem miniatura para {path}"));
            return;
        };
        {
            let state = app.state::<AppState>();
            let mut store = state.store.lock().unwrap();
            if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                e.thumb = Some(thumb.to_string_lossy().to_string());
            }
            store.save();
        }
        log(&format!("thumb: {path}"));
        let _ = app.emit("clipboard:changed", ());''',
'''        let st = app.state::<AppState>();
        let path = {
            let store = st.store.lock().unwrap();
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
            let mut store = state.store.lock().unwrap();
            store.put_thumb(&id, &bytes);
        }
        log(&format!("thumb: {path}"));
        let _ = app.emit("clipboard:changed", ());''')

# ---- link media dir ---------------------------------------------------------
rep('''        let (url, images_dir) = {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            match store.get(&id) {
                Some(e) if e.kind == "link" => {
                    let url = e.text.clone().unwrap_or_else(|| e.title.clone());
                    (url, store.images_dir())
                }
                _ => return,
            }
        };''',
'''        let (url, media) = {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            match store.get(&id) {
                Some(e) if e.kind == "link" => {
                    let url = e.text.clone().unwrap_or_else(|| e.title.clone());
                    (url, store.media_dir())
                }
                _ => return,
            }
        };''')

rep('''        let Some(meta) = linkmeta::fetch(&url, &images_dir) else {''',
'''        let Some(meta) = linkmeta::fetch(&url, &media) else {''')

# ---- delete ------
rep('''    {
        let mut store = state.store.lock().unwrap();
        if let Some(idx) = store.entries.iter().position(|e| e.id == id) {
            let removed = store.entries.remove(idx);
            if let Some(p) = removed.image_path {
                let _ = std::fs::remove_file(p);
            }
            store.save();
        }
    }''',
'''    {
        let mut store = state.store.lock().unwrap();
        store.remove(&id);
    }''')

# ---- clear ----
rep('''    {
        let mut store = state.store.lock().unwrap();
        let mut kept: Vec<Entry> = Vec::new();
        for e in store.entries.drain(..) {
            if keep_pinned && e.pinned {
                kept.push(e);
            } else if let Some(p) = e.image_path {
                let _ = std::fs::remove_file(p);
            }
        }
        store.entries = kept;
        store.save();
    }''',
'''    {
        let mut store = state.store.lock().unwrap();
        let kept: Vec<Entry> = store
            .entries
            .drain(..)
            .filter(|e| keep_pinned && e.pinned)
            .collect();
        store.entries = kept;
        store.save();
    }''')

# ---- source_of_clipboard: media/ em vez de images/ ------------------------
rep('''fn source_of_clipboard(images_dir: &std::path::Path) -> (Option<String>, Option<String>) {''',
'''fn source_of_clipboard(media_dir: &std::path::Path) -> (Option<String>, Option<String>) {''')
rep('''    let out = images_dir.join(format!("src-{stem}.png"));''',
'''    let out = media_dir.join(format!("src-{stem}.png"));''')
rep('''            let (source, source_icon) = {
                let images_dir = state.store.lock().unwrap().images_dir();
                source_of_clipboard(&images_dir)
            };
            let added = {
                let store = state.store.lock().unwrap();
                capture::build_entry(captured, &store, source, source_icon)
            };
            if let Some(entry) = added {
                let new_id = entry.id.clone();
                let is_image = entry.kind == "image";
                let is_new = {
                    let mut store = state.store.lock().unwrap();
                    let dup = store.entries.iter().any(|e| e.hash == entry.hash);
                    store.add(entry);
                    !dup
                };''',
'''            let (source, source_icon) = {
                let media_dir = state.store.lock().unwrap().media_dir();
                source_of_clipboard(&media_dir)
            };
            let added = {
                let store = state.store.lock().unwrap();
                capture::build_entry(captured, &store, source, source_icon)
            };
            if let Some(built) = added {
                let is_image = built.entry.kind == "image";
                let new_id = built.entry.id.clone();
                let is_new = {
                    let mut store = state.store.lock().unwrap();
                    let dup = store.entries.iter().any(|e| e.hash == built.entry.hash);
                    store.add(built.entry);
                    if let (Some(img), Some(thumb)) = (built.image, built.thumb) {
                        store.put_image(&new_id, &img, Some(&thumb));
                    } else if let Some(img) = built.image {
                        store.put_image(&new_id, &img, None);
                    }
                    !dup
                };''')

open(p, 'w', encoding='utf-8').write(s)
print('lib parte 2 ok')