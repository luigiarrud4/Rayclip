import re

# ---------- clipimg: variante em bytes ----------
p = 'src-tauri/src/clipimg.rs'
s = open(p, encoding='utf-8').read()
old = '''pub fn write_image(png_path: &std::path::Path) -> Result<(), String> {
    let img = image::open(png_path).map_err(|e| e.to_string())?;'''
new = '''/// Publica uma imagem que está em memória (bytes PNG/GIF/JPEG do banco).
pub fn write_image_bytes(bytes: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    publish(img)
}

pub fn write_image(png_path: &std::path::Path) -> Result<(), String> {
    let img = image::open(png_path).map_err(|e| e.to_string())?;
    publish(img)
}

fn publish(img: image::DynamicImage) -> Result<(), String> {'''
assert old in s
s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8').write(s)
print('clipimg ok')

# ---------- ocr: variante em bytes ----------
p = 'src-tauri/src/ocr.rs'
s = open(p, encoding='utf-8').read()
old = '''fn init_apartment() {'''
new = '''/// Lê o texto de uma imagem que está em memória (bytes do banco cifrado).
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
    let out = std::env::temp_dir().join(format!(
        "rayclip-ocr-{}-{}.png",
        std::process::id(),
        stamp
    ));
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

fn init_apartment() {'''
assert old in s
s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8').write(s)
print('ocr ok')
