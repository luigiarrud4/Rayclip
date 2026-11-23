p = 'src-tauri/src/lib.rs'
s = open(p, encoding='utf-8').read()


def rep(old, new, count=1):
    global s
    assert old in s, 'NAO ACHOU: ' + old[:90]
    s = s.replace(old, new, count)


# ---- módulos ---------------------------------------------------------------
rep('mod sqlite;\nmod thumbs;', 'mod key;\nmod sqlite;\nmod thumbs;')

# ---- copy: imagem vem do banco --------------------------------------------
rep('''    let entry = {
        let store = state.store.lock().unwrap();
        store.get(id).cloned()
    }
    .ok_or_else(|| "entry not found".to_string())?;

    capture::write_entry(&entry, plain_text)?;''',
'''    let (entry, image) = {
        let store = state.store.lock().unwrap();
        let entry = store
            .get(id)
            .cloned()
            .ok_or_else(|| "entry not found".to_string())?;
        let image = store.image_bytes(id);
        (entry, image)
    };

    capture::write_entry(&entry, image.as_deref(), plain_text)?;''')

# ---- image_source -> bytes ------------------------------------------------
rep('''fn image_source(state: &AppState, id: &str) -> Result<String, String> {
    let entry = {
        let store = state.store.lock().unwrap();
        store.get(id).cloned()
    }
    .ok_or_else(|| "entry not found".to_string())?;
    let src = entry
        .image_path
        .clone()
        .ok_or_else(|| "this entry has no image".to_string())?;
    if !std::path::Path::new(&src).exists() {
        return Err("image file is missing on disk".into());
    }
    Ok(src)
}''',
'''/// Bytes da imagem de uma entrada (o conteúdo fica no banco cifrado).
fn image_bytes_of(state: &AppState, id: &str) -> Result<(Vec<u8>, bool), String> {
    let store = state.store.lock().unwrap();
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
}''')

# ---- encode_image em bytes ------------------------------------------------
rep('''fn encode_image(src: &str, path: &str, ext: &str) -> Result<(), String> {
    let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
    let src_is_gif = src.to_lowercase().ends_with(".gif");
    let out: Vec<u8> = if src_is_gif && ext == "gif" {''',
'''fn encode_image(bytes: &[u8], is_gif: bool, path: &str, ext: &str) -> Result<(), String> {
    let out: Vec<u8> = if is_gif && ext == "gif" {''')

rep('''        let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let mut buf: Vec<u8> = Vec::new();''',
'''        let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        let mut buf: Vec<u8> = Vec::new();''')

# ---- save_entry_as: conteúdo vem do banco --------------------------------
rep('''    let ext = normalize_ext(&format, &entry);
    if entry.image_path.is_none() && entry.text.is_none() && entry.paths.is_empty() {
        return Err("this entry has nothing to save".into());
    }''',
'''    let ext = normalize_ext(&format, &entry);
    if !entry.has_image && entry.text.is_none() && entry.paths.is_empty() {
        return Err("this entry has nothing to save".into());
    }''')

rep('''    write_entry_as(&entry, std::path::Path::new(&path), &ext)?;
    log(&format!("save_entry_as {id} -> {path} ({ext})"));''',
'''    let (bytes, is_gif) = {
        let store = state.store.lock().unwrap();
        let img = if entry.has_image {
            store.image_bytes(&id)
        } else {
            None
        };
        let gif = entry.gif || img.as_deref().is_some_and(|b| b.starts_with(b"GIF8"));
        (img, gif)
    };
    write_entry_as(&entry, bytes.as_deref(), is_gif, std::path::Path::new(&path), &ext)?;
    log(&format!("save_entry_as {id} -> {path} ({ext})"));''')

# ---- save_image_as -------------------------------------------------------
rep('''    let src = image_source(&state, &id)?;
    let fmt = format.to_lowercase();
    let ext = match fmt.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "gif" => "gif",
        _ => "png",
    };
    let default_name = format!("Rayclip {}.{ext}", chrono_label());''',
'''    let (bytes, is_gif) = image_bytes_of(&state, &id)?;
    let fmt = format.to_lowercase();
    let ext = match fmt.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "gif" => "gif",
        _ => "png",
    };
    let default_name = format!("Rayclip {}.{ext}", chrono_label());''')

rep('''    encode_image(&src, &path, ext)?;
    log(&format!("save_image_as {id} -> {path} ({ext})"));''',
'''    encode_image(&bytes, is_gif, &path, ext)?;
    log(&format!("save_image_as {id} -> {path} ({ext})"));''')

# ---- save_image_to -------------------------------------------------------
rep('''    let src = image_source(&state, &id)?;
    let fmt = format.to_lowercase();
    let ext = match fmt.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "gif" => "gif",
        _ => "png",
    };
    encode_image(&src, &path, ext)?;
    Ok(path)''',
'''    let (bytes, is_gif) = image_bytes_of(&state, &id)?;
    let fmt = format.to_lowercase();
    let ext = match fmt.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "gif" => "gif",
        _ => "png",
    };
    encode_image(&bytes, is_gif, &path, ext)?;
    Ok(path)''')

# ---- default ext / stem / write_entry_as --------------------------------
rep('''fn entry_default_ext(entry: &crate::store::Entry) -> &'static str {
    if entry.image_path.is_some() {
        let p = entry.image_path.as_deref().unwrap_or("");
        return if p.to_lowercase().ends_with(".gif") { "gif" } else { "png" };
    }''',
'''fn entry_default_ext(entry: &crate::store::Entry) -> &'static str {
    if entry.has_image {
        return if entry.gif { "gif" } else { "png" };
    }''')

rep('''    let raw = if entry.image_path.is_some() {
        format!("Rayclip image {}", chrono_label())''',
'''    let raw = if entry.has_image {
        format!("Rayclip image {}", chrono_label())''')

rep('''fn write_entry_as(entry: &crate::store::Entry, path: &std::path::Path, ext: &str) -> Result<(), String> {
    if let Some(src) = entry.image_path.clone() {
        if src.to_lowercase().ends_with(".gif") && ext == "gif" {
            // preserva a animação copiando os bytes originais
            let bytes = std::fs::read(&src).map_err(|e| e.to_string())?;
            return std::fs::write(path, bytes).map_err(|e| e.to_string());
        }
        return encode_image(&src, &path.to_string_lossy(), ext);
    }''',
'''fn write_entry_as(
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
    }''')
open(p, 'w', encoding='utf-8').write(s)
print('lib parte 1 ok')
