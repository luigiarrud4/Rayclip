//! OCR de imagens do clipboard usando o motor nativo do Windows (WinRT Media.Ocr).
//!
//! Mesma ideia do Raycast: o texto reconhecido é guardado junto da entrada para
//! que a busca encontre imagens pelo conteúdo escrito nelas.

use std::path::{Path, PathBuf};

const MAX_OCR_SIDE: u32 = 2400;

/// Lê o texto de uma imagem em disco. Retorna `None` quando não há texto.
#[allow(dead_code)]
pub fn image_text(path: &Path) -> Option<String> {
    // WinRT exige um apartment inicializado nesta thread.
    init_apartment();

    let prepared = prepare(path)?;
    let text = recognize(&prepared);
    if prepared != path {
        let _ = std::fs::remove_file(&prepared);
    }
    let text = text?;
    let cleaned = text.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Lê o texto de uma imagem que está em memória (bytes do banco cifrado).
pub fn image_text_bytes(bytes: &[u8]) -> Option<String> {
    init_apartment();
    let img = image::load_from_memory(bytes).ok()?;
    let scaled = img.resize(
        MAX_OCR_SIDE,
        MAX_OCR_SIDE,
        image::imageops::FilterType::Lanczos3,
    );
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let out =
        std::env::temp_dir().join(format!("rayclip-ocr-{}-{}.png", std::process::id(), stamp));
    scaled.save(&out).ok()?;
    let text = recognize(&out);
    let _ = std::fs::remove_file(&out);
    let cleaned = text?.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Remove PNGs temporários deixados por execuções anteriores (um crash no meio
/// do OCR deixaria a imagem do clipboard em claro no `%TEMP%`).
pub fn cleanup_temp() {
    let dir = std::env::temp_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut n = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("rayclip-ocr-")
            && name.ends_with(".png")
            && std::fs::remove_file(e.path()).is_ok()
        {
            n += 1;
        }
    }
    if n > 0 {
        crate::log(&format!("ocr: {n} temporario(s) antigo(s) removido(s)"));
    }
}

fn init_apartment() {
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

/// O motor tem um limite de dimensão; imagens grandes são reduzidas antes.
#[allow(dead_code)]
fn prepare(path: &Path) -> Option<PathBuf> {
    let img = image::open(path).ok()?;
    if img.width() <= MAX_OCR_SIDE && img.height() <= MAX_OCR_SIDE {
        return Some(path.to_path_buf());
    }
    let scaled = img.resize(
        MAX_OCR_SIDE,
        MAX_OCR_SIDE,
        image::imageops::FilterType::Lanczos3,
    );
    let out = std::env::temp_dir().join(format!("rayclip-ocr-{}.png", std::process::id()));
    scaled.save(&out).ok()?;
    Some(out)
}

fn recognize(path: &Path) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::{FileAccessMode, StorageFile};

    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;
    let wide = HSTRING::from(path.to_string_lossy().as_ref());
    let file = StorageFile::GetFileFromPathAsync(&wide).ok()?.join().ok()?;
    let stream = file.OpenAsync(FileAccessMode::Read).ok()?.join().ok()?;
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.join().ok()?;
    let bitmap = decoder.GetSoftwareBitmapAsync().ok()?.join().ok()?;
    let result = engine.RecognizeAsync(&bitmap).ok()?.join().ok()?;
    let text = result.Text().ok()?.to_string_lossy();
    Some(text)
}
