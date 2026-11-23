//! OCR de imagens do clipboard usando o motor nativo do Windows (WinRT Media.Ocr).
//!
//! Mesma ideia do Raycast: o texto reconhecido é guardado junto da entrada para
//! que a busca encontre imagens pelo conteúdo escrito nelas.

use std::path::Path;

const MAX_OCR_SIDE: u32 = 2400;

/// Lê o texto de uma imagem em disco. Retorna `None` quando não há texto.
#[allow(dead_code)]
pub fn image_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    image_text_bytes(&bytes)
}

/// Lê o texto de uma imagem que está em memória (bytes do banco cifrado).
///
/// **Nada é gravado em disco.** O WinRT aceita um `InMemoryRandomAccessStream`:
/// a imagem reduzida é codificada em PNG na memória e decodificada de lá. A
/// versão anterior salvava um PNG em `%TEMP%`, o que deixava a imagem do
/// clipboard em claro no disco por alguns instantes (e para sempre após um
/// crash no meio do OCR).
pub fn image_text_bytes(bytes: &[u8]) -> Option<String> {
    init_apartment();
    let img = image::load_from_memory(bytes).ok()?;
    let scaled = img.resize(
        MAX_OCR_SIDE,
        MAX_OCR_SIDE,
        image::imageops::FilterType::Lanczos3,
    );
    let mut png: Vec<u8> = Vec::new();
    scaled
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    let text = recognize_bytes(&png);
    let cleaned = text?.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Remove PNGs temporários deixados por builds antigas (até a 0.1.25 o OCR
/// gravava a imagem em `%TEMP%`). Mantido só para limpar o que já existe.
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

fn recognize_bytes(png: &[u8]) -> Option<String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;
    // A imagem vive só na memória: nada de arquivo temporário em claro.
    let stream = InMemoryRandomAccessStream::new().ok()?;
    let writer = DataWriter::CreateDataWriter(&stream).ok()?;
    writer.WriteBytes(png).ok()?;
    let _ = writer.StoreAsync().ok()?.join().ok()?;
    let _ = writer.FlushAsync().ok()?.join().ok()?;
    let _ = writer.DetachStream().ok();
    stream.Seek(0).ok()?;
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.join().ok()?;
    let bitmap = decoder.GetSoftwareBitmapAsync().ok()?.join().ok()?;
    let result = engine.RecognizeAsync(&bitmap).ok()?.join().ok()?;
    let text = result.Text().ok()?.to_string_lossy();
    Some(text)
}
