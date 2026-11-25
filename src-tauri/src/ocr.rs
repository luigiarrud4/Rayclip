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
        // Restos de builds antigas: PNG do OCR (`rayclip-ocr-*`), pagina baixada
        // pelo curl (`rayclip-fetch-*.out`) e miniatura de arquivo
        // (`rayclip-thumb-*`). Um crash no meio de qualquer um deixaria o
        // arquivo para sempre no `%TEMP%`.
        let resto = name.starts_with("rayclip-ocr-") && name.ends_with(".png")
            || name.starts_with("rayclip-fetch-") && name.ends_with(".out")
            || name.starts_with("rayclip-thumb-") && name.ends_with(".png");
        if resto && std::fs::remove_file(e.path()).is_ok() {
            n += 1;
        }
    }
    if n > 0 {
        crate::log(&format!("limpeza: {n} temporario(s) antigo(s) removido(s)"));
    }
}

/// `true` quando este Windows tem o motor de OCR de verdade (a classe WinRT
/// `Windows.Media.Ocr.OcrEngine` registrada).
///
/// Em máquina sem o componente (VM enxuta, Windows sem o pacote de mídia), o
/// WinRT responde `Class not registered`: toda imagem vira "nenhum texto". A
/// interface precisa saber disso para dizer a verdade em vez de fingir que a
/// imagem não tem texto. Guardado em cache: o teste custa uma chamada COM.
pub fn disponivel() -> bool {
    static DISPONIVEL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DISPONIVEL.get_or_init(|| {
        init_apartment();
        motor().is_some()
    })
}

fn init_apartment() {
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

/// Motor de OCR.
///
/// `TryCreateFromUserProfileLanguages` devolve **erro** quando a lista de idiomas
/// do usuário está vazia (`Get-WinUserLanguageList` vazia, comum em instalação
/// enxuta) — e aí o app nunca reconhecia texto nenhum, com o pacote de OCR
/// instalado do lado. Por isso o plano B: pegar o primeiro idioma que tem
/// pacote de OCR (`AvailableRecognizerLanguages`), preferindo o do sistema.
fn motor() -> Option<windows::Media::Ocr::OcrEngine> {
    use windows::Media::Ocr::OcrEngine;

    if let Ok(e) = OcrEngine::TryCreateFromUserProfileLanguages() {
        return Some(e);
    }
    let langs = OcrEngine::AvailableRecognizerLanguages().ok()?;
    let n = langs.Size().unwrap_or(0);
    let mut escolhido = None;
    for i in 0..n {
        let Ok(l) = langs.GetAt(i) else { continue };
        if escolhido.is_none() {
            escolhido = Some(l.clone());
        }
        // O do sistema vem primeiro na lista; qualquer um serve, mas este é o
        // que acerta acentos do idioma da máquina.
        if l.LanguageTag().map(|t| t.to_string_lossy()).unwrap_or_default().starts_with("pt") {
            escolhido = Some(l);
            break;
        }
    }
    OcrEngine::TryCreateFromLanguage(&escolhido?).ok()
}

fn recognize_bytes(png: &[u8]) -> Option<String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
    let engine = motor()?;
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