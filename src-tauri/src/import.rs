//! Importa o histórico do Raycast original para o banco do Rayclip.
//!
//! O Raycast guarda o conteúdo em arquivos dentro de `%LOCALAPPDATA%\Raycast\clipboard`
//! (PNG, CF_HTML, RTF, TXT). O banco dele (`clipboard.db`) é SQLCipher com a
//! chave em DPAPI e não abre fora do processo dele — mas esses arquivos estão em
//! texto/imagem abertos, então é por aí que o histórico entra: cada arquivo virou
//! uma entrada nossa, com a data do arquivo e a imagem guardada no banco cifrado.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::store::{Entry, Store};

#[derive(Default, Debug)]
pub struct Report {
    pub total: usize,
    pub added: usize,
    pub skipped: usize,
    pub images: usize,
    pub texts: usize,
}

/// Transação do banco com rollback automático (Drop) - ver `run`.
struct Tx(Option<crate::sqlite::Db>);

impl Tx {
    fn begin(db: Option<crate::sqlite::Db>) -> Self {
        if let Some(d) = db.as_ref() {
            let _ = d.exec("BEGIN IMMEDIATE;");
        }
        Tx(db)
    }

    /// Fecha o lote a cada 200 itens e abre outro (não deixa um só de 1,4 GB).
    fn batch(&mut self, i: usize) {
        if i > 0 && i % 200 == 0 {
            if let Some(d) = self.0.as_ref() {
                let _ = d.exec("COMMIT;");
                let _ = d.exec("BEGIN IMMEDIATE;");
            }
        }
    }

    fn commit(mut self) {
        if let Some(d) = self.0.as_ref() {
            let _ = d.exec("COMMIT;");
        }
        self.0 = None;
    }
}

impl Drop for Tx {
    fn drop(&mut self) {
        if let Some(d) = self.0.as_ref() {
            let _ = d.exec("ROLLBACK;");
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Image,
    Html,
    Rtf,
    Txt,
}

#[derive(Clone)]
struct File {
    path: PathBuf,
    kind: Kind,
    mtime: i64,
}

/// Pasta onde o Raycast guarda o conteúdo do clipboard nesta máquina.
pub fn default_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let dir = base.join("Raycast").join("clipboard");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Quantos itens existem para importar (arquivos de um mesmo "copiar" contam 1).
pub fn count(dir: &Path) -> usize {
    grouped(scan(dir)).len()
}

fn scan(dir: &Path) -> Vec<File> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for e in read.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        let kind = match path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "png" | "jpg" | "jpeg" | "gif" | "webp" => Kind::Image,
            "html" | "htm" => Kind::Html,
            "rtf" => Kind::Rtf,
            "txt" => Kind::Txt,
            _ => continue,
        };
        let mtime = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        files.push(File { path, kind, mtime });
    }
    files.sort_by_key(|f| f.mtime);
    files
}

/// Agrupa arquivos do mesmo "copiar" (formatos diferentes, mesmo instante).
fn grouped(files: Vec<File>) -> Vec<((i64, bool), Vec<File>)> {
    let mut groups: BTreeMap<(i64, bool), Vec<File>> = BTreeMap::new();
    for f in files {
        // O Raycast grava cada formato com poucos ms de diferença: arredonda.
        let bucket = f.mtime / 2000 * 2000;
        // Imagem nunca se junta a texto (um print pode vir com HTML junto).
        // A chave é o par (instante, é_imagem) - antes era `bucket + 1`, que
        // podia colidir com o mtime real de uma imagem (perdida como duplicata)
        // e juntava TODAS as imagens copiadas dentro do mesmo bloco de 2 s.
        let key = (if f.kind == Kind::Image { f.mtime } else { bucket }, f.kind == Kind::Image);
        groups.entry(key).or_default().push(f);
    }
    groups.into_iter().collect()
}

pub fn run(
    app: &tauri::AppHandle,
    store: &mut Store,
    dir: &Path,
    progress: &mut dyn FnMut(usize, usize),
) {
    use tauri::Emitter;

    let groups = grouped(scan(dir));
    let total = groups.len();
    let mut report = Report {
        total,
        ..Default::default()
    };

    // Uma transação por lote: 1.400 itens sem isso seriam 1.400 fsyncs.
    // O guard fecha/desfaz a transação mesmo se algo der panic no meio:
    // transação aberta presa no processo trava o banco para sempre.
    let mut tx = Tx::begin(store.db());

    for (i, (_, files)) in groups.into_iter().enumerate() {
        tx.batch(i);
        let created_at = files.iter().map(|f| f.mtime).max().unwrap_or(0);
        let image = files.iter().find(|f| f.kind == Kind::Image);
        let html = files.iter().find(|f| f.kind == Kind::Html);
        let rtf = files.iter().find(|f| f.kind == Kind::Rtf);
        let txt = files.iter().find(|f| f.kind == Kind::Txt);

        let built = if let Some(img) = image {
            import_image(img, created_at)
        } else {
            import_text(txt, html, rtf, created_at)
        };
        if let Some((mut entry, blob)) = built {
            if store.entries.iter().any(|e| e.hash == entry.hash) {
                report.skipped += 1;
            } else {
                entry.created_at = created_at;
                let id = entry.id.clone();
                store.add(entry);
                if let Some(bytes) = blob {
                    store.put_image(&id, &bytes, None);
                    report.images += 1;
                } else {
                    report.texts += 1;
                }
                report.added += 1;
            }
        }
        if i % 100 == 0 || i + 1 == total {
            progress(i + 1, total);
            let _ = app.emit(
                "import:progress",
                serde_json::json!({ "done": i + 1, "total": total, "added": report.added }),
            );
        }
    }
    tx.commit();
    crate::log(&format!(
        "import: {} de {} itens ({} imagens, {} textos, {} repetidos)",
        report.added, report.total, report.images, report.texts, report.skipped
    ));
}

fn import_image(file: &File, created_at: i64) -> Option<(Entry, Option<Vec<u8>>)> {
    let bytes = std::fs::read(&file.path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let (w, h) = image::image_dimensions(&file.path).unwrap_or((0, 0));
    let id = uuid::Uuid::new_v4().to_string();
    // Mesmo hash da captura (bytes inteiros): a mesma imagem importada e
    // depois copiada de novo nao entra duas vezes.
    let hash = crate::hash_bytes(&[&b"image"[..], bytes.as_slice()]);
    Some((
        Entry {
            id,
            kind: "image".to_string(),
            title: String::new(),
            text: None,
            paths: vec![],
            has_image: true,
            image_width: Some(w),
            image_height: Some(h),
            source: Some("Raycast".to_string()),
            source_icon: None,
            ocr_text: None,
            has_thumb: false,
            gif: bytes.starts_with(b"GIF8"),
            link: None,
            html: None,
            rtf: None,
            size_bytes: bytes.len() as u64,
            created_at,
            pinned: false,
            pin_order: 0,
            hash,
        },
        Some(bytes),
    ))
}

fn import_text(
    txt: Option<&File>,
    html: Option<&File>,
    rtf: Option<&File>,
    created_at: i64,
) -> Option<(Entry, Option<Vec<u8>>)> {
    let raw_html = html.and_then(|f| std::fs::read_to_string(&f.path).ok());
    let plain = txt
        .and_then(|f| std::fs::read_to_string(&f.path).ok())
        .map(|s| s.replace("\r\n", "\n"))
        .filter(|s| !s.trim().is_empty())
        .or_else(|| raw_html.as_deref().and_then(cf_html_text))
        .filter(|s| !s.trim().is_empty())?;

    let kind = if crate::looks_like_link(&plain) {
        "link"
    } else if crate::capture::looks_like_color(&plain) {
        "color"
    } else if crate::capture::looks_like_email(&plain) {
        "email"
    } else {
        "text"
    };
    let id = uuid::Uuid::new_v4().to_string();
    let mut parts = vec!["text".to_string(), plain.clone()];
    if let Some(h) = raw_html.as_ref() {
        parts.push(h.clone());
    }
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    Some((
        Entry {
            id,
            kind: kind.to_string(),
            title: crate::capture::one_line(&plain),
            size_bytes: plain.len() as u64,
            text: Some(plain),
            paths: vec![],
            has_image: false,
            image_width: None,
            image_height: None,
            source: Some("Raycast".to_string()),
            source_icon: None,
            ocr_text: None,
            has_thumb: false,
            gif: false,
            link: None,
            html: raw_html,
            rtf: rtf.and_then(|f| std::fs::read_to_string(&f.path).ok()),
            created_at,
            pinned: false,
            pin_order: 0,
            hash: crate::hash_parts(&refs),
        },
        None,
    ))
}

/// Tira o texto legível de um bloco CF_HTML (o formato que o Windows clipboard usa).
fn cf_html_text(raw: &str) -> Option<String> {
    let frag_start = raw.find("<!--StartFragment-->").map(|i| i + 20);
    let frag_end = raw.find("<!--EndFragment-->");
    let body: &str = match (frag_start, frag_end) {
        (Some(s), Some(e)) if e > s => &raw[s..e],
        _ => {
            // Sem marcadores: usa os offsets do cabeçalho, se existirem.
            let off = |key: &str| -> Option<usize> {
                raw.lines().find_map(|line| {
                    line.strip_prefix(key)
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
            };
            match (off("StartFragment:"), off("EndFragment:")) {
                (Some(s), Some(e))
                    if e > s && e <= raw.len() && raw.is_char_boundary(s) && raw.is_char_boundary(e) =>
                {
                    &raw[s..e]
                }
                _ => raw,
            }
        }
    };
    strip_tags(body)
}

fn strip_tags(html: &str) -> Option<String> {
    let mut out = String::with_capacity(html.len() / 2);
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                // <br>, </p>, </div> viram quebra de linha; o resto some.
                let mut tag = String::new();
                for c2 in chars.by_ref() {
                    if c2 == '>' {
                        break;
                    }
                    tag.push(c2);
                }
                let t = tag.trim().to_lowercase();
                if t.starts_with("br") || t.starts_with("/p") || t.starts_with("/div") {
                    out.push('\n');
                }
            }
            '&' => {
                // Le o nome da entidade com `peek`: se nao for entidade
                // ("AT&T", "a & b"), o caractere seguinte volta para o fluxo
                // em vez de ser engolido (antes o espaco sumia e ainda
                // comia ate 8 letras antes de desistir).
                let mut ent = String::new();
                let mut closed = false;
                while let Some(&c2) = chars.peek() {
                    if c2 == ';' {
                        chars.next();
                        closed = true;
                        break;
                    }
                    if ent.len() >= 8
                        || c2.is_whitespace()
                        || c2 == '&'
                        || c2 == '<'
                        || c2 == '>'
                        || !c2.is_ascii()
                    {
                        break;
                    }
                    ent.push(c2);
                    chars.next();
                }
                let lower = ent.to_lowercase();
                let decoded = match lower.as_str() {
                    "nbsp" => Some(' '),
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    other if other.starts_with('#') => {
                        let n = if let Some(hex) = other.strip_prefix("#x") {
                            u32::from_str_radix(hex, 16).ok()
                        } else {
                            other[1..].parse::<u32>().ok()
                        };
                        n.and_then(char::from_u32)
                    }
                    _ => None,
                };
                match decoded {
                    Some(ch) => out.push(ch),
                    // Entidade invalida com ';': descarta.
                    None if closed => {}
                    None => {
                        out.push('&');
                        out.push_str(&ent);
                    }
                }
            }
            _ => out.push(c),
        }
    }
    // Linhas vazias demais nao interessam.
    let mut cleaned = String::with_capacity(out.len());
    let mut blanks = 0;
    for line in out.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::strip_tags;

    #[test]
    fn ampersand_alone_is_kept() {
        assert_eq!(strip_tags("AT&T Corp").unwrap(), "AT&T Corp");
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(strip_tags("a&amp;b &lt;c&gt;").unwrap(), "a&b <c>");
        assert_eq!(strip_tags("&#65;&#x42;").unwrap(), "AB");
    }

    #[test]
    fn tags_become_newlines() {
        assert_eq!(strip_tags("<p>um</p><p>dois</p>").unwrap(), "um\ndois");
        // o resultado final e trim()ado: a quebra inicial nao sobrevive
        assert_eq!(strip_tags("<br>a<br/>b").unwrap(), "a\nb");
        assert_eq!(strip_tags("um<br>dois<br/>tres").unwrap(), "um\ndois\ntres");
    }
}