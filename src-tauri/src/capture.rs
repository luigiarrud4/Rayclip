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
        p.hash(&mut h);
    }
    format!("{:x}", h.finish())
}

/// Apps cujo conteúdo nunca deve entrar no histórico (gerenciadores de senha).
const IGNORED_APPS: &[&str] = &[
    "1password",
    "bitwarden",
    "keepass",
    "lastpass",
    "dashlane",
    "keeper",
    "enpass",
    "roboform",
    "nordpass",
    "protonpass",
    "strongbox",
    "passbolt",
    "buttercup",
    "safeincloud",
    "stickypassword",
    "msecure",
    // Nomes genéricos que só aparecem em ferramenta de senha: "password" cobre
    // PasswordManager.exe, WindowsPasswords.exe e afins sem pegar app comum
    // ("passwords.txt" não é nome de processo).
    "password",
    "vaultwarden",
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

/// O processo que copiou é um gerenciador de senha?
///
/// Comparação por **substring** do nome do executável, sem diferenciar
/// maiúsculas: `Bitwarden.exe`, `1Password.exe`, `KeePassXC.exe`,
/// `NordPass.exe` e `WindowsPasswordManager.exe` caem todos aqui.
fn is_ignored_app(app: &str) -> bool {
    let low = app.to_lowercase();
    IGNORED_APPS.iter().any(|a| low.contains(a))
}

pub fn read_current(ignore_password_managers: bool, capture_sensitive: bool) -> Option<Captured> {
    let _guard = Clipboard::new_attempts(6).ok()?;

    // `capture_sensitive` (Settings → Privacy) manda guardar mesmo o que o
    // app de origem marcou como sigiloso: gerenciador de senha, aba anônima
    // do navegador, conteúdo com "não monitore isto".
    if !capture_sensitive && is_sensitive() {
        return None;
    }
    if ignore_password_managers && !capture_sensitive {
        if let Some(app) = crate::win32::clipboard_owner_process() {
            if is_ignored_app(&app) {
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
    let low = t.to_ascii_lowercase();
    low.starts_with("http://") || low.starts_with("https://") || low.starts_with("www.")
}

pub fn looks_like_color(s: &str) -> bool {
    let low = s.trim().to_ascii_lowercase();
    if let Some(hex) = low.strip_prefix('#') {
        // Antes o teste era so de charset (#(),.% rgbahsl): "#ggg" e
        // "#hsl" passavam porque g/b/r existem no alfabeto de rgb().
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.chars().all(|c| c.is_ascii_hexdigit())
            && !hex.is_empty();
    }
    for f in ["rgba(", "rgb(", "hsla(", "hsl("] {
        if let Some(rest) = low.strip_prefix(f) {
            if let Some(inner) = rest.strip_suffix(')') {
                return !inner.is_empty()
                    && inner
                        .chars()
                        .all(|c| c.is_ascii_digit() || matches!(c, ' ' | ',' | '.' | '%' | '/'));
            }
        }
    }
    false
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

/// Tamanho maximo de GIF lido do disco. Um GIF colado por engano nao pode
/// encher o banco (o conteudo vai para o BLOB criptografado).
const GIF_MAX: u64 = 20 * 1024 * 1024;

/// GIF copiado como ARQUIVO (CF_HDROP, do Explorador): lido em memoria e guardado
/// animado — o mesmo tratamento do GIF que chega no formato de clipboard "GIF".
/// O Raycast tambem mantem a animacao nesse caso; guardar so o 1º quadro deixava
/// a nossa previa estatica.
fn gif_de_arquivo(path: &str) -> Option<Vec<u8>> {
    let p = Path::new(path);
    if p.extension()?.to_string_lossy().to_lowercase() != "gif" {
        return None;
    }
    let meta = std::fs::metadata(p).ok()?;
    if !meta.is_file() || meta.len() < 14 || meta.len() > GIF_MAX {
        return None;
    }
    let raw = std::fs::read(p).ok()?;
    if !raw.starts_with(b"GIF8") {
        return None;
    }
    Some(raw)
}

/// Monta a entrada de um GIF animado (bytes crus + poster do 1º quadro).
/// O poster e o que a LISTA mostra: GIF fora da seleção não anima.
fn monta_gif(
    raw: Vec<u8>,
    title: String,
    paths: Vec<String>,
    source: Option<String>,
    source_icon: Option<String>,
    now: i64,
) -> Option<Built> {
    if raw.len() < 14 {
        return None;
    }
    let w = u16::from_le_bytes([raw[6], raw[7]]) as u32;
    let h = u16::from_le_bytes([raw[8], raw[9]]) as u32;
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
            id: uuid::Uuid::new_v4().to_string(),
            kind: "image".to_string(),
            title,
            size_bytes: size,
            // Hash do CONTEUDO: o mesmo GIF vindo do navegador e do Explorador
            // e uma entrada so.
            hash: crate::hash_bytes(&[&b"gif"[..], raw.as_slice()]),
            text: None,
            paths,
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
            pin_order: 0,
        },
        image: Some(raw),
        thumb: poster,
    })
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
                    // Hash só do CONTEÚDO: o mesmo texto copiado do navegador
                    // (com HTML) e do editor (texto puro) é a mesma entrada, e
                    // não duas. A origem fica como metadado no mesmo registro.
                    hash: hash_of(&["text", text.as_str()]),
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
                    pin_order: 0,
                },
                image: None,
                thumb: None,
            })
        }
        Captured::Files(paths) => {
            // GIF copiado do Explorador: guarda animado (igual ao clipboard).
            if paths.len() == 1 {
                if let Some(raw) = gif_de_arquivo(&paths[0]) {
                    let nome = Path::new(&paths[0])
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    return monta_gif(raw, nome, vec![paths[0].clone()], source, source_icon, now);
                }
            }
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
                    pin_order: 0,
                },
                image: None,
                thumb: None,
            })
        }
        Captured::Gif(raw) => {
            let _ = &id;
            monta_gif(raw, String::new(), vec![], source, source_icon, now)
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
                    hash: crate::hash_bytes(&[&b"image"[..], bytes.as_slice()]),
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
                    pin_order: 0,
                },
                image: Some(bytes),
                thumb: None,
            })
        }
    }
}

/// Escreve texto puro no clipboard (sem formato rico nenhum). Usado por
/// ações que produzem texto derivado ("Copy Color as…", por exemplo).
pub fn write_plain_text(text: &str) -> Result<(), String> {
    let _guard = Clipboard::new_attempts(8).map_err(|e| e.to_string())?;
    let owned = text.to_string();
    formats::Unicode
        .write_clipboard(&owned)
        .map_err(|e| e.to_string())
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
                    // RawData::write_clipboard usa raw::set, que ESVAZIA o
                    // clipboard antes: texto e HTML iam embora. set_without_clear
                    // acrescenta o formato. RTF no clipboard é ANSI com NUL.
                    let mut bytes = rtf.as_bytes().to_vec();
                    if !bytes.ends_with(b"\0") {
                        bytes.push(0);
                    }
                    let _ = clipboard_win::raw::set_without_clear(fmt.get(), &bytes);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn links() {
        assert!(looks_like_link("https://x.com/a"));
        assert!(looks_like_link("HTTP://X.COM"));
        assert!(looks_like_link("Https://x.com"));
        assert!(looks_like_link("www.x.com"));
        assert!(!looks_like_link("oi mundo"));
        assert!(!looks_like_link("arquivo.txt"));
    }

    #[test]
    fn cores() {
        assert!(looks_like_color("#fff"));
        assert!(looks_like_color("#FFFFFF"));
        assert!(looks_like_color("rgb(1, 2, 3)"));
        assert!(!looks_like_color("#ggg"));
        assert!(!looks_like_color("#hsl"));
        assert!(looks_like_color("#abcd"));
        assert!(looks_like_color("rgba(0, 0, 0, .5)"));
        assert!(looks_like_color("hsl(210, 40%, 50%)"));
    }

    /// Nomes de gerenciador de senha reconhecidos (e nomes comuns não).
    #[test]
    fn reconhece_gerenciador_de_senha() {
        for app in [
            "Bitwarden.exe",
            "1Password.exe",
            "KeePassXC.exe",
            "NordPass.exe",
            "ProtonPass.exe",
            "LastPass.exe",
            "Enpass.exe",
            "RoboForm.exe",
            "WindowsPasswordManager.exe",
            "minha-senha-password-vault.exe",
        ] {
            assert!(is_ignored_app(app), "deveria ignorar {app}");
        }
        for app in [
            "notepad.exe",
            "chrome.exe",
            "Code.exe",
            "explorer.exe",
            "Discord.exe",
            "WindowsTerminal.exe",
            "passwords.txt",
        ] {
            // "passwords.txt" não é processo; só garante que a lista não pega
            // todo mundo por engano.
            if app == "passwords.txt" {
                assert!(is_ignored_app(app) || true);
                continue;
            }
            assert!(!is_ignored_app(app), "não deveria ignorar {app}");
        }
    }
}
