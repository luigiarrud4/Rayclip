import re

p = 'src-tauri/src/capture.rs'
s = open(p, encoding='utf-8').read()

def rep(old, new, count=1):
    global s
    assert old in s, old[:70]
    s = s.replace(old, new, count)

rep('''pub fn build_entry(
    captured: Captured,
    store: &Store,
    source: Option<String>,
    source_icon: Option<String>,
) -> Option<Entry> {''',
'''/// O que a captura produz: a entrada e (quando houver) a imagem para o banco.
pub struct Built {
    pub entry: Entry,
    pub image: Option<Vec<u8>>,
    pub thumb: Option<Vec<u8>>,
}

pub fn build_entry(
    captured: Captured,
    store: &Store,
    source: Option<String>,
    source_icon: Option<String>,
) -> Option<Built> {''')

# ---- texto ----
rep('''            Some(Entry {
                id,
                kind: kind.to_string(),
                title: one_line(&text),
                size_bytes: text.len() as u64,
                thumb: None,''',
'''            Some(Built {
                entry: Entry {
                id,
                kind: kind.to_string(),
                title: one_line(&text),
                size_bytes: text.len() as u64,
                has_thumb: false,''')

rep('''                text: Some(text),
                html,
                rtf,
                paths: vec![],
                image_path: None,
                image_width: None,
                image_height: None,
                source,
                source_icon,
                ocr_text: None,
                created_at: now,
                pinned: false,
            })''',
'''                text: Some(text),
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
            })''')

# ---- arquivos ----
rep('''            Some(Entry {
                id,
                kind,
                title,
                size_bytes: size,
                hash: hash_of(&["files", &joined]),
                text: None,
                paths,
                image_path: None,
                image_width: None,
                image_height: None,
                source,
                source_icon,
                ocr_text: None,
                thumb: None,''',
'''            Some(Built {
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
                has_thumb: false,''')

rep('''                link: None,
                html: None,
                rtf: None,
                created_at: now,
                pinned: false,
            })
        }
        Captured::Gif(raw) => {''',
'''                link: None,
                html: None,
                rtf: None,
                created_at: now,
                pinned: false,
                },
                image: None,
                thumb: None,
            })
        }
        Captured::Gif(raw) => {''')

# ---- GIF ----
rep('''            let file = store.images_dir().join(format!("{id}.gif"));
            std::fs::write(&file, &raw).ok()?;
            // Poster estático (1º quadro): a lista usa ele, então GIFs fora da
            // seleção não ficam animando e gastando CPU/GPU.
            let mut poster = None;
            if let Ok(frame) = image::load_from_memory(&raw) {
                let p = store.images_dir().join(format!("{id}-poster.png"));
                let small = frame.resize(320, 320, image::imageops::FilterType::Triangle);
                if small.save(&p).is_ok() {
                    poster = Some(p.to_string_lossy().to_string());
                }
            }
            let size = raw.len() as u64;
            Some(Entry {''',
'''            // Poster estático (1º quadro): a lista usa ele, então GIFs fora da
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
                entry: Entry {''')

rep('''                hash: hash_of(&[
                    "gif",
                    &crate::linkmeta::short_hash(&file.to_string_lossy()),
                    &size.to_string(),
                ]),
                text: None,
                paths: vec![],
                image_path: Some(file.to_string_lossy().to_string()),
                image_width: Some(w),
                image_height: Some(h),
                source,
                source_icon,
                ocr_text: None,
                thumb: poster,
                link: None,
                html: None,
                rtf: None,
                created_at: now,
                pinned: false,
            })''',
'''                hash: hash_of(&[
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
                link: None,
                html: None,
                rtf: None,
                created_at: now,
                pinned: false,
                },
                image: Some(raw),
                thumb: poster,
            })''')

# ---- imagem colada ----
rep('''            let file = store.images_dir().join(format!("{id}.png"));
            img.save(&file).ok()?;
            let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            Some(Entry {''',
'''            let mut cursor = std::io::Cursor::new(Vec::new());
            img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
            let bytes = cursor.into_inner();
            let size = bytes.len() as u64;
            Some(Built {
                entry: Entry {''')

rep('''                hash: hash_of(&[
                    "image",
                    &hash_of(&[&file.to_string_lossy()]),
                    &w.to_string(),
                    &h.to_string(),
                    &size.to_string(),
                ]),
                text: None,
                paths: vec![],
                image_path: Some(file.to_string_lossy().to_string()),
                image_width: Some(w),
                image_height: Some(h),
                source,
                source_icon,
                ocr_text: None,
                thumb: None,
                link: None,
                html: None,
                rtf: None,
                created_at: now,
                pinned: false,
            })''',
'''                hash: hash_of(&[
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
                link: None,
                html: None,
                rtf: None,
                created_at: now,
                pinned: false,
                },
                image: Some(bytes),
                thumb: None,
            })''')

# ---- write_entry recebe os bytes ----
rep('''pub fn write_entry(entry: &Entry, plain_text: bool) -> Result<(), String> {''',
'''pub fn write_entry(
    entry: &Entry,
    image: Option<&[u8]>,
    plain_text: bool,
) -> Result<(), String> {''')

rep('''    if let Some(p) = &entry.image_path {
        if !plain_text {
            return crate::clipimg::write_image(std::path::Path::new(p));
        }
    }''',
'''    if let Some(bytes) = image {
        if !plain_text {
            return crate::clipimg::write_image_bytes(bytes);
        }
    }''')

open(p, 'w', encoding='utf-8').write(s)
print('capture ok; image_path=', 'image_path' in s)
