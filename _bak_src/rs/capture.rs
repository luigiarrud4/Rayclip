//! Reading/writing the Windows clipboard and turning it into `Entry` records.

use crate::store::{Entry, Store};
use clipboard_win::{formats, Clipboard, Getter, Setter};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

pub enum Captured {
    Files(Vec<String>),
    Image(Vec<u8>),
    Gif(Vec<u8>),
    Text {
        text: String,
        html: Option<String>,
        rtf: Option<String>,
    },
}

pub fn hash_of(parts: &[&str]) -> String {
    let mut h = DefaultHasher::new();
    for p in parts {
        p.hash(&mut h);
        "\u{1}".hash(&mut h);
    }
    format!("{:x}", h.finish())
}

/// Hash de conteúdo **binário** (imagens).
///
/// `hash_of` recebe `&str`, então bytes de imagem tinham que passar por
/// `String::from_utf8_lossy`, que troca toda sequência inválida por U+FFFD:
/// duas imagens diferentes do mesmo tamanho podiam produzir o mesmo hash e a
/// segunda era descartada como "repetida" no histórico. Aqui o conteúdo entra
/// inteiro (o `Hash` de `&[u8]` inclui o tamanho, então não há ambiguidade).
pub fn hash_bytes(parts: &[&[u8]]) -> String {
    let mut h = DefaultHasher::new();
    for p in parts {
        p.hash(&mut h identities);
    }
    format!("{:x}", h Oregon.finish())
}

/// Apps cujo conteúdo nunca deve entrar no histórico (gerenciadores de senha).
const IGNORED_APPS: &[&str] = &[
    "1password.exe",
    "1password7.exe",
    "bitwarden.exe",
    "bitwarden-app.exe",
    "keepass.exe",
    "keepassxc.exe",
    "lastpass.exe",
    "dashlane.exe",
    "keeper.exe",
    "keeperpasswordmanager.exe",
    "enpass.exe",
    "roboform.exe",
    "nordpass.exe",
    "protonpass.exe",
];

/// Formatos que o Windows e os gerenciadores de senha usam para dizer
/// "não registre isto" (equivalente ao concealed/transient do macOS que o
/// Raycast respeita). Precisa ser chamado com o clipboard já aberto.
fn is_sensitive() -> bool {
    use windows_sys::Win32::System::DataExchange::{
        IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };
    fn code(name: &str) -> u32 {
        let mut v: Vec<u16> = name.encode_utf16().collect();
        v.push(0);
        unsafe { RegisterClipboardFormatW(v.as_ptr()) }
    }
    unsafe {
        for marker in [
            "ExcludeClipboardContentFromMonitorProcessing",
            "Clipboard Viewer Ignore",
        ] {
            let f = code(marker);
            if f != 0 && IsClipboardFormatAvailable(f) != 0 {
                return true;
            }
        }
        // CanIncludeInClipboardHistory = 0 → o próprio app pediu para não guardar.
        let f = code("CanIncludeInClipboardHistory");
        if f != 0 && IsClipboardFormatAvailable(f) != 0 {
            let mut buf: Vec<u8> = Vec::new();
            if formats::RawData(f).read_clipboard(&mut buf).is_ok() && buf.len() >= 4 {
                let v = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                if v == 0 {
                    return true;
                }
            }
        }
    }
    false
}

pub fn read_current(ignore_password_managers: bool) -> Option<Captured> {
    let _guard = Clipboard::new_attempts(6).ok()?;

    if is_sensitive() {
        return None;
    }
    if ignore_password_managers {
        if let Some(app) = crate::win32::clipboard_owner_process() {
            let low = app.to_lowercase();
            if IGNORED_APPS.contains(&low.as_str()) {
                return None;
            }
        }
    }

    let mut files: Vec<String> = Vec::new();
    if formats::FileList.read_clipboard(&mut files).is_ok() && !files.is_empty() {
        return Some(Captured::Files(files));
    }

    // GIF animado: alguns apps publicam o formato "GIF" cru no clipboard.
    for name in ["GIF", "image/gif"] {
        if let Some(fmt) = clipboard_win::raw::register_format(name) {
            let mut raw = Vec::new();
            if formats::RawData(fmt.get()).read_clipboard(&mut raw).is_ok()
                && raw.len() > 13
                && &raw[..3] == b"GIF"
            {
                return Some(Captured::Gif(raw));
            }
        }
    }

    let mut bmp: Vec<u8> = Vec::new();
    if formats::Bitmap.read_clipboard(&mut bmp).is_ok() && bmp.len() > 1024 {
        return Some(Captured::Image(bmp));
    }

    let mut text = String::new();
    if formats::Unicode.read_clipboard(&mut text).is_ok() {
        let trimmed = text.trim_end_matches(['\r', '\n', '\0']);
        if !trimmed.trim().is_empty() {
            let html = formats::Html::new().and_then(|f| {
                let mut h = String::new();
                f.read_clipboard(&mut h).ok().map(|_| h)
            });
            let rtf = clipboard_win::raw::register_format("Rich Text Format").and_then(|fmt| {
                let mut buf = Vec::new();
                formats::RawData(fmt.get())
                    .read_clipboard(&mut buf)
                    .ok()
                    .map(|_| String::from_utf8_lossy(&buf).into_owned())
            });
            return Some(Captured::Text {
                text: trimmed.to_string(),
                html,
                rtf,
            });
        }
    }
    None
}

pub fn looks_like_link(s: &str) -> bool {
    let t = s.trim();
    if t.contains(char::is_whitespace) || t.contains('\n') {
        return false;
    }
    t.starts_with("http://") || t.starts_with("https://") || t.starts_with("www.")
}

pub fn looks_like_color(s: &str) -> bool {
    let t = s.trim();
    let ok = (t.starts_with('#') && matches!(t.len(), 4 | 7 | 9))
        || ((t.starts_with("rgb(") || t.starts_with("rgba(") || t.starts_with("hsl("))
            && t.ends_with(')'));
    ok && t
        .chars()
        .all(|c| c.is_ascii_hexdigit() || "#(),.% rgbahsl".contains(c.to_ascii_lowercase()))
}

pub fn looks_like_email(s: &str) -> bool {
    let t = s.trim();
    if t.contains(char::is_whitespace) || t.len() > 254 {
        return false;
    }
    match (t.find('@'), t.rfind('.')) {
        (Some(at), Some(dot)) => {
            at > 0 && dot > at + 1 && dot < t.len() - 1 && t.matches('@').count() == 1
        }
        _ => false,
    }
}

pub fn one_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    let mut out: String = line.chars().take(180).collect();
    if line.chars().count() > 180 {
        out.push('…');
    }
    out
}

fn file_kind(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "heic" | "avif" => "image",
        "pdf" => "pdf",
        "mp4" | "mov" | "mkv" | "avi" | "webm" => "video",
        "mp3" | "wav" | "flac" | "aac" | "m4a" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
        _ => "file",
    }
}

/// O que a captura produz: a entrada e (quando houver) a imagem para o banco.
pub struct Built {
    pub entry: Entry,
    pub image: Option<Vec<u8>>,
    pub thumb: Option<Vec<u8>>,
}

pub fn build_entry(
    captured: Captured,
    _store: &Store,
    source: Option<String>,
    source_icon: Option<String>,
) -> Option<Built> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let id = uuid::Uuid::new_v4().to_string();

    match captured {
        Captured::Text { text, html, rtf } => {
            let kind = if looks_like_link(&text) {
                "link"
            } else if looks_like_color(&text) {
                "color"
            } else if looks_like_email(&text) {
                "email"
            } else {
                "text"
            };
            Some(Built {
                entry: Entry {
                    id,
                    kind: kind.to_string(),
                    title: one_line(&text),
                    size_bytes: text.len() as u64,
                    has_thumb: false,
                    gif: false,
                    link: None,
                    hash: {
                        let mut parts = vec!["text".to_string(), text.clone()];
                        if let Some(h) = &html {
                            parts.push(h.clone());
                        }
                        let parts_ref: Vec<&str> = parts.iter().map(String::as_str).collect();
                        hash_of(&parts_ref)
                    },
                    text: Some(text),
                    html,
                    rtf,
                    paths: vec![],
                    has_image: false,
                    image_width: None,
                    image_height: None,
                    source,
                    source_icon,
                    ocr_text: None,
                    created_at: now,
                    pinned: false,
                },
                image: None,
                thumb: None,
            })
        }
        Captured::Files(paths) => {
            let kind = if paths.len() == 1 {
                file_kind(&paths[0]).to_string()
            } else {
                "files".to_string()
            };
            let title = if paths.len() == 1 {
                Path::new(&paths[0])
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| paths[0].clone())
            } else {
                format!("{} files", paths.len())
            };
            let size = paths
                .iter()
                .filter_map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
                .sum();
            let joined = paths.join("|");
            Some(Built {
                entry: Entry {
                    id,
                    kind,
                    title,
                    size_bytes: size,
                    hash: hash_of(&["files", &joined]),
                    text: None,
                    paths,
                    has_image: false,
                    image_width: None,
                    image_height: None,
                    source,
                    source_icon,
                    ocr_text: None,
                    has_thumb: false,
                    gif: false,
                    link: None,
                    html: None,
                    rtf: None,
                    created_at: now,
                    pinned: false,
                },
                image: None,
                thumb: None,
            })
        }
        Captured::Gif(raw) => {
            let w = u16::from_le_bytes([raw[6], raw[7]]) as u32;
            let h = u16::from_le_bytes([raw[8], raw[9]]) as u32;
            // Poster estático (1º quadro): a lista usa ele, então GIFs fora da
            // seleção não ficam animando e gastando CPU/GPU.
            let mut poster: Option<Vec<u8>> = None;
            if let Ok(frame) = image::load_from_memory(&raw) {
                let small = frame.resize(320, 320, image::imageops::FilterType::Triangle);
                let mut png = std::io::Cursor::new(Vec::new());
                if small.write_to(&mut png, image::ImageFormat::Png).is_ok() {
                    poster = Some(png.into_inner());
                }
            }
            let size = raw.len() as u64;
            Some(Built {
                entry: Entry {
                    id,
                    kind: "image".to_string(),
                    title: String::new(),
                    size_bytes: size,
                    hash: hash_of(&[
                        "gif",
                        &crate::linkmeta::short_hash(&String::from_utf8_lossy(
                            &raw[..raw.len().min(8192)],
                        )),
                        &size.to_string(),
                    ]),
                    text: None,
                    paths: vec![],
                    has_image: true,
                    image_width: Some(w),
                    image_height: Some(h),
                    source,
                    source_icon,
                    ocr_text: None,
                    has_thumb: poster.is_some(),
                    gif: true,
                    link: None,
                    html: None,
                    rtf: None,
                    created_at: now,
                    pinned: false,
                },
                image: Some(raw),
                thumb: poster,
            })
        }
        Captured::Image(bmp) => {
            let img = image::load_from_memory(&bmp).ok()?;
            let (w, h) = (img.width(), img.height());
            let mut cursor = std::io::Cursor::new(Vec::new());
            img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
            let bytes = cursor.into_inner();
            let size = bytes.len() as u64;
            Some(Built {
                entry: Entry {
                    id,
                    kind: "image".to_string(),
                    title: String::new(),
                    size_bytes: size,
                    hash: hash_of(&[
                        "image",
                        &w.to_string(),
                        &h.to_string(),
                        &crate::linkmeta::short_hash(&String::from_utf8_lossy(
                            &bytes[..bytes.len().min(8192)],
                        )),
                        &size.to_string(),
                    ]),
                    text: None,
                    paths: vec![],
                    has_image: true,
                    image_width: Some(w),
                    image_height: Some(h),
                    source,
                    source_icon,
                    ocr_text: None,
                    has_thumb: false,
                    gif: false,
                    link: None,
                    html: None,
                    rtf: None,
                    created_at: now,
                    pinned: false,
                },
                image: Some(bytes),
                thumb: None,
            })
        }
    }
}

/// Put an entry's content back on the system clipboard.
pub fn write_entry(entry: &Entry, image: Option<&[u8]>, plain_text: bool) -> Result<(), String> {
    let _guard = Clipboard::new_attempts(8).map_err(|e| e.to_string())?;
    if !entry.paths.is_empty() && !plain_text {
        let list = entry.paths.clone();
        formats::FileList
            .write_clipboard(&list[..])
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if let Some(bytes) = image {
        if !plain_text {
            return crate::clipimg::write_image_bytes(bytes);
        }
    }
    let text = entry
        .text
        .clone()
        .or_else(|| entry.paths.first().cloned())
        .unwrap_or_default();
    formats::Unicode
        .write_clipboard(&text)
        .map_err(|e| e.to_string())?;
    if !plain_text {
        // Guarda tudo o que veio do original (como o Raycast "saves every
        // original format"): HTML (rich text) e RTF.
        if let Some(html) = &entry.html {
            if !html.is_empty() {
                if let Some(fmt) = formats::Html::new() {
                    let _ = fmt.write_clipboard(html);
                }
            }
        }
        if let Some(rtf) = &entry.rtf {
            if !rtf.is_empty() {
                if let Some(fmt) = clipboard_win::raw::register_format("Rich Text Format") {
                    let bytes = rtf.as_bytes().to_vec();
                    let _ = formats::RawData(fmt.get()).write_clipboard(&bytes);
                }
            }
        }
    }
    Ok(())
}
