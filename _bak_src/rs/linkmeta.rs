//! Metadados de links, como o Raycast mostra: título do site, favicon e capa
//! (Open Graph). Busca uma única vez e guarda em cache no próprio registro.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub site_name: Option<String>,
    /// Favicon em data URL (pequeno, cabe no JSON).
    pub icon: Option<String>,
    /// Capa (og:image) como data URL — vai cifrada dentro do banco, não em disco.
    pub image_path: Option<String>,
    pub url: Option<String>,
}

fn http_get(url: &str, max: usize) -> Option<(String, Vec<u8>)> {
    use std::io::Read;
    let agent = ureq::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(8))
        .build();
    let resp = agent.get(url).call().ok()?;
    let ctype = resp.header("content-type").unwrap_or("").to_string();
    let mut buf: Vec<u8> = Vec::new();
    resp.into_reader()
        .take(max as u64)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.is_empty() {
        return None;
    }
    Some((ctype, buf))
}

fn norm_url(u: &str) -> String {
    if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{u}")
    }
}

fn resolve(base: &str, href: &str) -> String {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let (scheme, rest) = match base.split_once("://") {
        Some((s, r)) => (s, r),
        None => ("https", base),
    };
    if let Some(stripped) = href.strip_prefix("//") {
        return format!("{scheme}://{stripped}");
    }
    let origin_end = rest.find('/').unwrap_or(rest.len());
    let origin = &rest[..origin_end];
    if href.starts_with('/') {
        return format!("{scheme}://{origin}{href}");
    }
    let path = &rest[origin_end..];
    let dir = match path.rfind('/') {
        Some(i) => &path[..=i],
        None => "/",
    };
    format!("{scheme}://{origin}{dir}{href}")
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=");
    let i = find_ci(tag, &needle)? + needle.len();
    let rest = &tag[i..];
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)? + 1;
        Some(rest[1..end].to_string())
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let entity = &rest[1..end];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn meta_content(html: &str, keys: &[&str]) -> Option<String> {
    for key in keys {
        let mut from = 0usize;
        while let Some(i) = find_ci(&html[from..], key) {
            let pos = from + i;
            let start = html[..pos].rfind('<')?;
            let end = html[pos..].find('>').map(|e| pos + e)?;
            let tag = &html[start..end];
            if let Some(v) = attr_value(tag, "content") {
                let v = unescape(v.trim());
                if !v.is_empty() {
                    return Some(v);
                }
            }
            from = end.max(pos + 1);
        }
    }
    None
}

fn html_title(html: &str) -> Option<String> {
    let i = find_ci(html, "<title")?;
    let open_end = html[i..].find('>').map(|e| i + e + 1)?;
    let close = find_ci(&html[open_end..], "</title")? + open_end;
    let t = unescape(html[open_end..close].trim());
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn find_icon(html: &str) -> Option<String> {
    let mut from = 0usize;
    while let Some(i) = find_ci(&html[from..], "<link") {
        let s = from + i;
        let e = html[s..].find('>').map(|x| s + x)?;
        let tag = &html[s..e];
        let rel = attr_value(tag, "rel").unwrap_or_default().to_lowercase();
        if rel.contains("icon") {
            if let Some(href) = attr_value(tag, "href") {
                if !href.trim().is_empty() {
                    return Some(href);
                }
            }
        }
        from = e.max(s + 1);
    }
    None
}

fn sniff_ext(bytes: &[u8], ctype: &str) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("png");
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 11 && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.starts_with(&[0, 0, 1, 0]) {
        return Some("ico");
    }
    if let Some(t) = ctype.split(';').next() {
        let t = t.trim().to_lowercase();
        if t.contains("svg") {
            return Some("svg");
        }
        for (key, ext) in [
            ("image/png", "png"),
            ("image/jpeg", "jpg"),
            ("image/gif", "gif"),
            ("image/webp", "webp"),
            ("image/x-icon", "ico"),
            ("image/vnd.microsoft.icon", "ico"),
        ] {
            if t == key {
                return Some(ext);
            }
        }
    }
    None
}

fn mime_of(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/x-icon",
    }
}

fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

/// Extrai o id de um vídeo do YouTube a partir dos formatos de URL comuns.
pub fn youtube_id(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1).unwrap_or(url).to_string();
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let valid = |s: &str| {
        !s.is_empty()
            && s.len() <= 24
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    if host.ends_with("youtu.be") {
        let id = rest.split('/').nth(1)?.split(['?', '#']).next()?;
        return valid(id).then(|| id.to_string());
    }
    if !host.contains("youtube.") {
        return None;
    }
    let tail = rest.split_once('/')?.1;
    if let Some(q) = tail.split_once("v=").map(|(_, b)| b) {
        let id = q.split('&').next().unwrap_or(q);
        if valid(id) {
            return Some(id.to_string());
        }
    }
    for prefix in ["shorts/", "embed/", "live/", "v/"] {
        if let Some(rest) = tail.strip_prefix(prefix) {
            let id = rest.split(['?', '&', '/']).next().unwrap_or(rest);
            if valid(id) {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Hash curto e estável para nome de arquivo da capa.
pub fn short_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Busca (uma vez) os metadados de um link. A capa vem como data URL (nada é
/// gravado em disco: o JSON vai direto para o banco cifrado).
pub fn fetch(url: &str) -> Option<LinkMeta> {
    let url = norm_url(url.trim());
    let (_ctype, body) = http_get(&url, 700_000)?;
    let html = String::from_utf8_lossy(&body).into_owned();

    let mut meta = LinkMeta {
        url: Some(url.clone()),
        title: meta_content(&html, &["og:title", "twitter:title"]).or_else(|| html_title(&html)),
        description: meta_content(
            &html,
            &[
                "og:description",
                "name=\"description\"",
                "twitter:description",
            ],
        ),
        site_name: meta_content(&html, &["og:site_name"]).or_else(|| Some(host_of(&url))),
        icon: None,
        image_path: None,
    };
    if let Some(d) = &meta.description {
        let d: String = d.chars().take(300).collect();
        meta.description = Some(d);
    }
    if let Some(t) = &meta.title {
        let t: String = t.chars().take(200).collect();
        meta.title = Some(t);
    }

    // favicon
    let icon_url = find_icon(&html)
        .map(|h| resolve(&url, &h))
        .unwrap_or_else(|| resolve(&url, "/favicon.ico"));
    if let Some((ct, bytes)) = http_get(&icon_url, 40_000) {
        if let Some(ext) = sniff_ext(&bytes, &ct) {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            meta.icon = Some(format!("data:{};base64,{b64}", mime_of(ext)));
        }
    }

    // capa: og:image ou, para vídeos, a miniatura do provedor
    let cover =
        meta_content(&html, &["og:image:secure_url", "og:image", "twitter:image"]).or_else(|| {
            youtube_id(&url).map(|id| format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
        });
    if let Some(img) = cover {
        let img_url = resolve(&url, &img);
        if let Some((ct, bytes)) = http_get(&img_url, 900_000) {
            if let Some(ext) = sniff_ext(&bytes, &ct) {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                meta.image_path = Some(format!("data:{};base64,{b64}", mime_of(ext)));
            }
        }
    }

    Some(meta)
}
