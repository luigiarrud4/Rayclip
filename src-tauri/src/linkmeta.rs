//! Metadados de links, como o Raycast mostra: título do site, favicon e capa
//! (Open Graph). Busca uma única vez e guarda em cache no próprio registro.

use serde::{Deserialize, Serialize};

/// Favicon maior que isso é raro; acima do limite o ICO chega truncado.
const ICON_MAX: usize = 400_000;

/// Versão do buscador de metadados (ver `LinkMeta::fetch_v`).
pub const FETCH_VERSION: u32 = 5;

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
    /// Já tentamos buscar metadados e o site não entregou título (bloqueio de
    /// bot, página só de JS). Evita re-tentar a mesma URL a cada abertura.
    #[serde(default)]
    pub tried: bool,
    /// Versão do buscador que gravou isto. Quando o buscador melhora (mais
    /// cabeçalhos, plano B pelo curl), a versão sobe e as entradas sem título
    /// ganham **uma** nova tentativa, em vez de ficarem presas no fracasso.
    #[serde(default)]
    pub fetch_v: Option<u32>,
}

/// Destino aceitavel para uma requisicao automatica?
///
/// O Rayclip busca metadados de qualquer link copiado - inclusive um link que
/// outra pessoa escreveu. Sem esta barreira, copiar
/// `http://192.168.0.1/admin` (ou `http://localhost:8080`) faria o app bater
/// dentro da rede local: o classico SSRF. So passa **http/https** para um host
/// que resolva **exclusivamente** para enderecos publicos.
/// Teto para capa de **mídia direta** (GIF de tenor/giphy costuma ter 1–8 MB;
/// og:image de página fica nos 900 KB de antes).
const MIDIA_MAX: usize = 8 * 1024 * 1024;

/// Guarda os bytes da capa em `media/covers/<hash da URL>` e devolve a URL do
/// protocolo próprio. Bytes **crus**: um GIF animado continua animado (é o que
/// faz o link colado aparecer se mexendo, como no Discord).
fn guarda_capa(media: &std::path::Path, img_url: &str, bytes: &[u8]) -> Option<String> {
    let key = cover_key(img_url)?;
    let dir = media.join("covers");
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::write(dir.join(&key), bytes).ok()?;
    Some(format!("http://rayclip.localhost/cover/{key}"))
}

/// A URL é um arquivo de mídia direto (`.gif`, `.webp`, `.png`, `.mp4`…)?
///
/// Link colado que **é** a mídia, não uma página: um `.../gato.gif` do tenor.
/// A extensão é o sinal barato; o `Content-Type` confirma depois.
fn extensao_de_midia(url: &str) -> Option<&'static str> {
    let sem_query = url.split(['?', '#']).next().unwrap_or(url);
    let ext = sem_query.rsplit('.').next()?.to_lowercase();
    match ext.as_str() {
        "gif" | "webp" | "png" | "jpg" | "jpeg" | "avif" => Some("imagem"),
        "mp4" | "webm" | "mov" => Some("video"),
        _ => None,
    }
}

pub fn url_permitida(url: &str) -> bool {
    let Some((esquema, resto)) = url.split_once("://") else {
        return false;
    };
    let esquema = esquema.to_ascii_lowercase();
    if esquema != "http" && esquema != "https" {
        return false;
    }
    let autoridade = resto.split(['/', '?', '#']).next().unwrap_or("");
    // sem userinfo (`http://usuario:senha@host/`)
    let host_porta = match autoridade.rsplit_once('@') {
        Some((_, h)) => h,
        None => autoridade,
    };
    let host = if let Some(fim) = host_porta.strip_prefix('[') {
        match fim.split_once(']') {
            Some((h, _)) => h.to_string(),
            None => return false,
        }
    } else {
        host_porta.split(':').next().unwrap_or("").to_string()
    };
    host_publico(&host)
}

fn host_publico(host: &str) -> bool {
    use std::net::{IpAddr, ToSocketAddrs};
    let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() {
        return false;
    }
    for sufixo in [
        ".localhost",
        ".local",
        ".internal",
        ".lan",
        ".home.arpa",
        ".intranet",
    ] {
        if h == "localhost" || h.ends_with(sufixo) {
            return false;
        }
    }
    if let Ok(ip) = h.parse::<IpAddr>() {
        return ip_publico(ip);
    }
    // Nome de maquina da rede local (sem dominio) ou forma numerica estranha
    // (`127.1`, `0x7f.0.0.1`) nao passam.
    if !h.contains('.') || h.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return false;
    }
    // Resolve agora: se **qualquer** endereco for interno, o host inteiro cai
    // (evita nome publico apontando para 127.0.0.1). Nao resolver = nao pedir.
    match (h.as_str(), 0u16).to_socket_addrs() {
        Ok(addrs) => {
            let mut vistos = 0;
            for a in addrs {
                vistos += 1;
                if !ip_publico(a.ip()) {
                    return false;
                }
            }
            vistos > 0
        }
        Err(_) => false,
    }
}

fn ip_publico(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr::{V4, V6};
    match ip {
        V4(v4) => {
            let o = v4.octets();
            !(o[0] == 0                                          // 0.0.0.0/8
                || o[0] == 10                                     // 10/8 privado
                || (o[0] == 100 && (64..128).contains(&o[1]))     // 100.64/10 CGNAT
                || o[0] == 127                                    // loopback
                || (o[0] == 169 && o[1] == 254)                   // link-local
                || (o[0] == 172 && (16..32).contains(&o[1]))      // 172.16/12
                || (o[0] == 192 && o[1] == 168)                   // 192.168/16
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)        // 192.0.0/24 IETF
                || (o[0] == 192 && o[1] == 0 && o[2] == 2)        // documentacao
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))    // benchmark
                || (o[0] == 198 && o[1] == 51 && o[2] == 100)     // documentacao
                || (o[0] == 203 && o[1] == 0 && o[2] == 113)      // documentacao
                || o[0] >= 224)                                   // multicast/reservado
        }
        V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return ip_publico(V4(v4));
            }
            let s = v6.segments();
            !(v6.is_loopback()                        // ::1
                || v6.is_unspecified()                // ::
                || s[0] & 0xff00 == 0xff00            // ff00::/8 multicast
                || s[0] & 0xfe00 == 0xfc00            // fc00::/7 unique local
                || s[0] & 0xffc0 == 0xfe80            // fe80::/10 link local
                || s[0] & 0xffc0 == 0xfec0            // fec0::/10 site local
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentacao
                || s[0] == 0x2002                     // 6to4 (encapsula IPv4)
                || (s[0] == 0x2001 && s[1] == 0))     // Teredo
        }
    }
}

/// GET/HEAD com a guarda aplicada **a cada salto**: o `Location` de um 3xx e
/// revalidado antes de ser seguido (um site publico pode redirecionar para
/// `http://127.0.0.1`). Devolve a URL efetiva e a resposta final.
fn pedir(url: &str, head: bool, html: bool) -> Option<(String, ureq::Response)> {
    const MAX_SALTOS: usize = 5;
    let mut atual = norm_url(url);
    for salto in 0..=MAX_SALTOS {
        if !url_permitida(&atual) {
            crate::log(&format!(
                "link: destino nao publico bloqueado ({} salto {})",
                host_of(&atual),
                salto
            ));
            return None;
        }
        let req = if head {
            agent().head(&atual)
        } else {
            agent().get(&atual)
        };
        let req = if html {
            req.set(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .set("Accept-Language", "pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7")
            .set("Upgrade-Insecure-Requests", "1")
        } else {
            req
        };
        let resp = req.call().ok()?;
        if !(300..400).contains(&resp.status()) {
            return Some((atual, resp));
        }
        let local = resp.header("location")?.to_string();
        atual = resolve(&atual, &local);
    }
    None
}

fn http_get(url: &str, max: usize) -> Option<(String, Vec<u8>)> {
    use std::io::Read;
    let (_, resp) = pedir(url, false, false)?;
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

/// Só o cabeçalho: evita baixar centenas de KB de um CDN só para descobrir
/// que o link não é uma página HTML.
fn head_ct(url: &str) -> Option<String> {
    let (_, resp) = pedir(url, true, false)?;
    Some(resp.header("content-type").unwrap_or("").to_string())
}

fn http_get_final(url: &str, max: usize) -> Option<(String, String, Vec<u8>)> {
    use std::io::Read;
    let (final_url, resp) = pedir(url, false, false)?;
    let ctype = resp.header("content-type").unwrap_or("").to_string();
    let mut buf: Vec<u8> = Vec::new();
    resp.into_reader()
        .take(max as u64)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.is_empty() {
        return None;
    }
    Some((final_url, ctype, buf))
}

fn is_html(ctype: &str) -> bool {
    let c = ctype.to_ascii_lowercase();
    c.contains("text/html") || c.contains("application/xhtml")
}

/// "cdn.discordapp.com" -> "discordapp.com" (mantém sufixos compostos).
fn registrable(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() <= 2 {
        return host.to_string();
    }
    const TWO: [&str; 12] = [
        "co.uk", "com.br", "com.au", "co.jp", "co.kr", "com.mx", "com.ar", "com.tr", "co.in",
        "com.cn", "com.pt", "co.za",
    ];
    let last = parts[parts.len() - 2..].join(".");
    let take = if TWO.contains(&last.as_str()) { 3 } else { 2 };
    parts[parts.len() - take..].join(".")
}

/// Arquivo de favicon por host: `media/fav/<host>` (conteúdo cru, sem extensão).
fn fav_file(media: &std::path::Path, host: &str) -> Option<std::path::PathBuf> {
    let safe: String = host
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
        .collect();
    if safe.is_empty() {
        return None;
    }
    let dir = media.join("fav");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(safe))
}

/// Garante o favicon do site em disco e devolve a URL local servida pelo app.
/// Ordem: host -> domínio registrável -> www do registrável. Guarda em cache,
/// então cada site é baixado uma única vez (e o `offline & private` continua
/// valendo, porque a lista nunca busca nada na rede).
pub fn favicon_url(media: &std::path::Path, url: &str) -> Option<String> {
    let host = host_of(url);
    if host.is_empty() {
        return None;
    }
    let file = fav_file(media, &host)?;
    if std::fs::metadata(&file).map(|m| m.len() > 0).unwrap_or(false) {
        return Some(format!("http://rayclip.localhost/fav/{host}"));
    }
    let root = registrable(&host);
    let mut candidates = vec![host.clone()];
    if root != host {
        candidates.push(root.clone());
        candidates.push(format!("www.{root}"));
    }
    // caminhos alternativos comuns quando /favicon.ico não existe
    let paths = ["favicon.ico", "favicon.png", "apple-touch-icon.png"];
    // Primeiro só cabeçalhos (HEAD); baixa o primeiro que existir de fato.
    let mut fallback: Option<String> = None;
    for c in &candidates {
        for p in paths {
            let u = format!("https://{c}/{p}");
            let Some(ct) = head_ct(&u) else { continue };
            if !ct.to_ascii_lowercase().starts_with("image/") {
                continue;
            }
            if let Some((ct2, bytes)) = http_get(&u, ICON_MAX) {
                if sniff_ext(&bytes, &ct2).is_some() && std::fs::write(&file, &bytes).is_ok() {
                    return Some(format!("http://rayclip.localhost/fav/{host}"));
                }
            }
            if fallback.is_none() {
                fallback = Some(u);
            }
        }
    }
    // Servidor que não responde a HEAD: tenta um GET direto no primeiro candidato.
    if let Some(u) = fallback {
        if let Some((ct, bytes)) = http_get(&u, ICON_MAX) {
            if sniff_ext(&bytes, &ct).is_some() && std::fs::write(&file, &bytes).is_ok() {
                return Some(format!("http://rayclip.localhost/fav/{host}"));
            }
        }
    }
    // Sem /favicon.ico na raiz (ex.: cdn.discordapp.com não tem): lê a página
    // inicial do domínio e usa o `<link rel="icon">` que ela declara.
    for c in &candidates {
        let home = format!("https://{c}/");
        let Some((final_url, ct, body)) = http_get_final(&home, 300_000) else {
            continue;
        };
        if !is_html(&ct) {
            continue;
        }
        let html = String::from_utf8_lossy(&body);
        let Some(href) = find_icon(&html) else { continue };
        let u = resolve(&final_url, &href);
        let Some((ct2, bytes)) = http_get(&u, ICON_MAX) else {
            continue;
        };
        if sniff_ext(&bytes, &ct2).is_some() && std::fs::write(&file, &bytes).is_ok() {
            return Some(format!("http://rayclip.localhost/fav/{host}"));
        }
    }
    None
}

fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Nome do arquivo de capa: hash da URL (estável, sem depender da entrada).
pub fn cover_key(url: &str) -> Option<String> {
    let u = url.trim();
    if u.is_empty() {
        return None;
    }
    Some(format!("{:016x}", fnv64(u)))
}

/// Agente HTTP único: UA de navegador **e** os cabeçalhos que os sites usam
/// para decidir se a requisição é de gente. Sem `Accept`/`Accept-Language`, a
/// Pichau (Cloudflare) respondia 403 e o título nunca chegava.
fn agent() -> ureq::Agent {
    ureq::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(8))
        // Sem redirecionamento automatico: `pedir()` segue os saltos um a um,
        // validando o destino de cada um (anti-SSRF).
        .redirects(0)
        .build()
}

/// Título de página de bloqueio/desafio (Cloudflare, WAF, manutenção) não é o
/// título do conteúdo — é melhor ficar sem título e mostrar a URL legível.
pub fn titulo_ruim(t: &str) -> bool {
    let s = t.trim().to_ascii_lowercase();
    if s.is_empty() {
        return true;
    }
    const RUINS: [&str; 8] = [
        "just a moment",
        "attention required",
        "access denied",
        "are you a robot",
        "site em manuten",
        "403 forbidden",
        "404 not found",
        "cloudflare",
    ];
    RUINS.iter().any(|r| s.contains(r))
}

fn get_html(url: &str) -> Option<ureq::Response> {
    pedir(url, false, true).map(|(_, r)| r)
}

/// Metadados mínimos quando a página não responde (bloqueio, timeout, 403):
/// host + favicon do domínio já bastam para a linha e o cartão não ficarem
/// genéricos — e a entrada deixa de ser re-tentada a cada abertura.
pub fn minimal(url: &str, media: &std::path::Path) -> LinkMeta {
    let url = norm_url(url.trim());
    LinkMeta {
        url: Some(url.clone()),
        site_name: Some(host_of(&url)),
        icon: resolve_icon(media, &url, None),
        tried: true,
        fetch_v: Some(FETCH_VERSION),
        ..Default::default()
    }
}

/// Ícone do link: guarda os bytes do `<link rel="icon">` no cache por host e
/// devolve a URL local; sem ícone declarado, cai no favicon do domínio.
pub fn resolve_icon(media: &std::path::Path, page_url: &str, href: Option<String>) -> Option<String> {
    let host = host_of(page_url);
    if let (Some(h), Some(file)) = (href.as_deref(), fav_file(media, &host)) {
        let exists = std::fs::metadata(&file).map(|m| m.len() > 0).unwrap_or(false);
        if !exists {
            let u = resolve(page_url, h);
            if let Some((ct, bytes)) = http_get(&u, ICON_MAX) {
                if sniff_ext(&bytes, &ct).is_some() {
                    let _ = std::fs::write(&file, &bytes);
                    return Some(format!("http://rayclip.localhost/fav/{host}"));
                }
            }
        }
    }
    favicon_url(media, page_url)
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
    let b = tag.as_bytes();
    let needle = attr.as_bytes();
    if needle.is_empty() || b.len() < needle.len() {
        return None;
    }
    let mut start = None;
    for i in 0..=b.len() - needle.len() {
        if !b[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            continue;
        }
        // "data-content" nao e "content": exige limite de nome antes.
        if i > 0 {
            let p = b[i - 1];
            if p.is_ascii_alphanumeric() || p == b'-' || p == b'_' || p == b':' {
                continue;
            }
        }
        let mut j = i + needle.len();
        // `content = "x"` (espaco antes do =) e valido em HTML.
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == b'=' {
            start = Some(j + 1);
            break;
        }
    }
    let mut j = start?;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= b.len() {
        return None;
    }
    let q = b[j];
    if q == b'"' || q == b'\'' {
        let from = j + 1;
        let end = b[from..].iter().position(|c| *c == q)? + from;
        Some(String::from_utf8_lossy(&b[from..end]).into_owned())
    } else if q == b'>' {
        None
    } else {
        let end = b[j..]
            .iter()
            .position(|c| c.is_ascii_whitespace() || *c == b'>')
            .map(|d| j + d)
            .unwrap_or(b.len());
        Some(String::from_utf8_lossy(&b[j..end]).into_owned())
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
            // Um '<' ausente aqui nao pode abortar TODAS as buscas
            // (era o `?`): so esta ocorrencia da chave e invalida.
            let start = match html[..pos].rfind('<') {
                Some(v) => v,
                None => {
                    from = pos + 1;
                    continue;
                }
            };
            let end = match html[pos..].find('>') {
                Some(e) => pos + e,
                None => break,
            };
            let tag = &html[start..end];
            if let Some(v) = attr_value(tag, "content") {
                let v = unescape(v.trim());
                if !v.is_empty() {
                    return Some(v);
                }
            }
            from = end + 1;
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
    let mut best: Option<(u8, String)> = None;
    while let Some(i) = find_ci(&html[from..], "<link") {
        let s = from + i;
        // `<link>` sem '>' fecha o documento: para, nao abandona o que ja achou.
        let e = match html[s..].find('>') {
            Some(v) => s + v,
            None => break,
        };
        let tag = &html[s..e];
        let rel = attr_value(tag, "rel").unwrap_or_default().to_lowercase();
        // apple-touch-icon costuma ser o arquivo maior/melhor; um
        // rel="icon" generico pode apontar para um .ico de 16x16.
        let rank = if rel.contains("apple-touch-icon") {
            3
        } else if rel.contains("icon") {
            2
        } else {
            0
        };
        if rank > 0 {
            if let Some(href) = attr_value(tag, "href") {
                let melhor = match &best {
                    Some(b) => rank > b.0,
                    None => true,
                };
                if !href.trim().is_empty() && melhor {
                    best = Some((rank, href));
                }
            }
        }
        from = e + 1;
    }
    best.map(|(_, href)| href)
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

#[allow(dead_code)]
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

/// Título de vídeo/canal do YouTube via oEmbed: JSON pequeno, estável e sem
/// HTML. Devolve `None` quando não é YouTube ou a API não responde (aí o HTML
/// ainda pode trazer `og:title`).
fn youtube_title(url: &str) -> Option<String> {
    let host = host_of(url);
    if !(host.contains("youtube.") || host == "youtu.be") {
        return None;
    }
    let api = format!(
        "https://www.youtube.com/oembed?url={}&format=json",
        urlencode(url)
    );
    let (ct, bytes) = http_get(&api, 60_000)?;
    if !ct.to_ascii_lowercase().contains("json") {
        return None;
    }
    let json = String::from_utf8_lossy(&bytes).into_owned();
    let raw = json_field(&json, "title")?;
    let v = raw.trim().to_string();
    (!v.is_empty()).then_some(v)
}

/// Valor de um campo string de primeiro nível do JSON (o oEmbed é plano e
/// pequeno, então isto basta). Desfaz os escapes que aparecem em títulos,
/// inclusive pares surrogados de emoji.
fn json_field(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":");
    let i = json.find(&pat)? + pat.len();
    let rest = json[i..].trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut it = rest.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'u' => {
                    let hex: String = it.by_ref().take(4).collect();
                    let cp = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
                    if (0xD800..0xDC00).contains(&cp) {
                        let mut vit = it.clone();
                        if vit.next() == Some('\\') && vit.next() == Some('u') {
                            let hex2: String = vit.by_ref().take(4).collect();
                            if let Ok(low) = u32::from_str_radix(&hex2, 16) {
                                if (0xDC00..0xE000).contains(&low) {
                                    let full = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                    out.push(char::from_u32(full).unwrap_or('\u{fffd}'));
                                    it = vit;
                                    continue;
                                }
                            }
                        }
                    }
                    out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// Codificação mínima para query string (`:` e `/` viram `%3A`/`%2F`).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// curl.exe do proprio Windows como plano B: o TLS de rustls tem impressao
/// digital propria e alguns sites (Cloudflare — a Pichau, por exemplo) devolvem
/// 403 para ele. O curl.exe existe desde o Windows 10 1803 e passa nessas
/// paginas; o WinINet do sistema, testado aqui, recebia a pagina de manutencao.
fn curl_get(url: &str, max: usize) -> Option<(String, Vec<u8>)> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // A resposta vai pelo stdout, sem arquivo temporario: a pagina baixada
    // (que pode ser a propria pagina que o usuario copiou) nao fica em claro
    // em `%TEMP%` nem por um instante.
    const MARCA: &str = "
__RAYCLIP_CT__";

    // O curl e um processo separado: nao da para ver os saltos de um `-L`, entao
    // ele **nao segue redirecionamento** (e so fala http/https). Link encurtado
    // que cai aqui fica sem metadados - o ureq, que valida cada salto, cobre o
    // caso comum.
    if !url_permitida(url) {
        return None;
    }
    let out = Command::new("curl.exe")
        .args([
            "-sS",
            "--max-redirs",
            "0",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--max-time",
            "9",
            "--max-filesize",
            "3000000",
            "--compressed",
            "-A",
            UA,
            "-H",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "-H",
            "Accept-Language: pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7",
            "-w",
            &format!("{MARCA}%{{content_type}}"),
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    // O corpo vem antes da marca; o tipo de conteudo vem depois dela.
    let (corpo, ct) = match rfind(&out.stdout, MARCA.as_bytes()) {
        Some(i) => (
            out.stdout[..i].to_vec(),
            String::from_utf8_lossy(&out.stdout[i + MARCA.len()..])
                .trim()
                .to_string(),
        ),
        None => (out.stdout.clone(), String::new()),
    };
    if corpo.is_empty() {
        return None;
    }
    Some((ct, corpo.into_iter().take(max).collect()))
}

/// Última ocorrência de `needle` em `hay` (o corpo pode conter a marca).
fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len())
        .rev()
        .find(|&i| hay[i..i + needle.len()] == *needle)
}

/// Página HTML: ureq primeiro (rápido, sem dependência); se o site recusar o
/// cliente Rust (403), tenta de novo pelo curl do Windows.
fn page_html(url: &str, max: usize) -> Option<(String, Vec<u8>)> {
    if let Some(resp) = get_html(url) {
        use std::io::Read;
        let ct = resp.header("content-type").unwrap_or("").to_string();
        let mut body: Vec<u8> = Vec::new();
        if resp
            .into_reader()
            .take(max as u64)
            .read_to_end(&mut body)
            .is_ok()
            && !body.is_empty()
        {
            return Some((ct, body));
        }
    }
    curl_get(url, max)
}

/// Busca (uma vez) os metadados de um link. A capa vem como data URL (nada é
/// gravado em disco: o JSON vai direto para o banco cifrado).
pub fn fetch(url: &str, media: &std::path::Path) -> Option<LinkMeta> {
    let url = norm_url(url.trim());

    // Link direto de arquivo (CDN, PDF, imagem): não é página HTML — nada de
    // baixar centenas de KB nem de procurar <meta>. Só host + favicon do site.
    let ct = head_ct(&url).unwrap_or_default();
    if !ct.is_empty() && !is_html(&ct) {
        let mut meta = LinkMeta {
            url: Some(url.clone()),
            site_name: Some(host_of(&url)),
            icon: resolve_icon(media, &url, None),
            tried: true,
            fetch_v: Some(FETCH_VERSION),
            ..Default::default()
        };
        // Link colado que **é** a mídia (tenor/giphy/IMGUR direto): baixa e
        // guarda como capa. É o "só mandar o link e o GIF aparece" do Discord —
        // e como os bytes vão crus para o disco, o GIF anima na tela.
        let parece_imagem = ct.starts_with("image/") || extensao_de_midia(&url) == Some("imagem");
        if parece_imagem && !url_permitida(&url) {
            return Some(meta);
        }
        if parece_imagem {
            if let Some((ct2, bytes)) = http_get(&url, MIDIA_MAX) {
                let conhecida = sniff_ext(&bytes, &ct2).is_some();
                if conhecida {
                    meta.image_path = guarda_capa(media, &url, &bytes);
                }
            }
        }
        return Some(meta);
    }

    let (ctype, body): (String, Vec<u8>) = page_html(&url, 2_500_000)?;
    if !ctype.is_empty() && !is_html(&ctype) {
        // Cabeçalho dizia HTML mas veio um arquivo (CDN): linha com host+ícone.
        return Some(LinkMeta {
            url: Some(url.clone()),
            site_name: Some(host_of(&url)),
            icon: resolve_icon(media, &url, None),
            tried: true,
            fetch_v: Some(FETCH_VERSION),
            ..Default::default()
        });
    }
    if body.is_empty() {
        return None;
    }
    let html = String::from_utf8_lossy(&body).into_owned();

    let mut meta = LinkMeta {
        url: Some(url.clone()),
        // YouTube: o oEmbed entrega o título limpo ("ABSURDO! Eu trouxe o GTA 6
        // pro meu PC") em vez de "<title> - YouTube"; a página, sem cookies de
        // consentimento, às vezes vem vazia de metadados.
        title: youtube_title(&url)
            .or_else(|| meta_content(&html, &["og:title", "twitter:title"]))
            .or_else(|| html_title(&html)),
        tried: true,
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
        fetch_v: Some(FETCH_VERSION),
    };
    if let Some(d) = &meta.description {
        let d: String = d.chars().take(300).collect();
        meta.description = Some(d);
    }
    if let Some(t) = &meta.title {
        let t: String = t.chars().take(200).collect();
        meta.title = (!titulo_ruim(&t)).then_some(t);
    }

    // favicon: guardado em disco, servido por `http://rayclip.localhost/fav/<host>`
    // (antes ia como data URL em cada entrada, engordando toda listagem).
    meta.icon = resolve_icon(media, &url, find_icon(&html));

    // capa: og:image ou, para vídeos, a miniatura do provedor
    // Capa: og:image da página; para YouTube, também as miniaturas do vídeo
    // (a página sem cookies às vezes vem sem metadados). Lista de candidatas,
    // em ordem de qualidade — a primeira que baixar vira a prévia.
    let mut candidatos: Vec<String> = Vec::new();
    if let Some(img) = meta_content(&html, &["og:image:secure_url", "og:image", "twitter:image"]) {
        candidatos.push(resolve(&url, &img));
    }
    if let Some(id) = youtube_id(&url) {
        candidatos.push(format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg"));
        candidatos.push(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"));
    }
    for img_url in candidatos {
        let Some((ct, bytes)) = http_get(&img_url, 900_000) else {
            continue;
        };
        if sniff_ext(&bytes, &ct).is_none() {
            continue;
        }
        // Guarda em disco e serve por URL: um data URL de capa (até 900 KB)
        // entraria na listagem inteira, a cada abertura.
        if let Some(url_capa) = guarda_capa(media, &img_url, &bytes) {
            meta.image_path = Some(url_capa);
            break;
        }
    }

    Some(meta)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Link que **é** a mídia (GIF do tenor, print no imgur) é reconhecido pela
    /// extensão; página normal não é.
    #[test]
    fn extensao_de_midia_reconhece_gif_e_video() {
        assert_eq!(extensao_de_midia("https://media.tenor.com/x/gato.gif"), Some("imagem"));
        assert_eq!(
            extensao_de_midia("https://i.imgur.com/abc.PNG?size=large"),
            Some("imagem")
        );
        assert_eq!(extensao_de_midia("https://cdn.x/v.mp4"), Some("video"));
        assert_eq!(extensao_de_midia("https://exemplo.com/artigo"), None);
        assert_eq!(extensao_de_midia("https://exemplo.com/a.b/c"), None);
    }

    #[test]
    fn attr_aceita_espaco_ao_redor_do_igual() {
        let t = concat!("<meta", " property='og:title' content = 'Oi' >");
        assert_eq!(attr_value(t, "content").as_deref(), Some("Oi"));
    }

    #[test]
    fn attr_nao_confunde_data_content() {
        let t = concat!("<meta data-", "content=\"errado\" content=\"certo\">");
        assert_eq!(attr_value(t, "content").as_deref(), Some("certo"));
    }

    #[test]
    fn attr_sem_aspas_e_case_insensitive() {
        let t = concat!("<META CONTENT=abc", "def>");
        assert_eq!(attr_value(t, "content").as_deref(), Some("abcdef"));
    }

    #[test]
    fn meta_content_ignora_trecho_sem_menor_que() {
        let html = concat!("texto solto og:title aqui ", "<meta ", "property='og:title' ", "content='Titulo'>");
        assert_eq!(meta_content(html, &["og:title"]).as_deref(), Some("Titulo"));
    }

    #[test]
    fn unescape_cobre_hex_e_nomeados() {
        assert_eq!(unescape("a &amp; b &#x42; &#65;"), "a & b B A");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enderecos que nunca podem receber requisicao automatica (SSRF).
    #[test]
    fn bloqueia_destinos_internos() {
        let maus = [
            "http://127.0.0.1/",
            "http://127.0.0.1:8080/admin",
            "https://localhost/",
            "http://localhost:1420/",
            "http://LOCALHOST./",
            "http://192.168.0.1/admin",
            "http://192.168.15.1:8000/",
            "http://10.1.2.3/",
            "http://172.20.0.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.64.0.1/",
            "http://0.0.0.0/",
            "http://[::1]/",
            "http://[fe80::1]/",
            "http://[fd00::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://intranet/",
            "http://servidor.lan/",
            "http://printer.local/",
            "http://api.internal/",
            "http://router.home.arpa/",
            "http://127.1/",
            "ftp://example.com/",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "http://user:senha@127.0.0.1/",
            "http://2130706433/",
        ];
        for u in maus {
            assert!(!url_permitida(u), "deveria bloquear {u}");
        }
    }

    /// Enderecos publicos (literais, sem DNS) passam.
    #[test]
    fn aceita_destinos_publicos() {
        let bons = [
            "http://8.8.8.8/",
            "https://1.1.1.1/icon.png",
            "https://8.8.4.4:8443/x?y=1",
            "http://[2606:4700:4700::1111]/",
        ];
        for u in bons {
            assert!(url_permitida(u), "deveria aceitar {u}");
        }
    }

    /// O corpo do `curl_get` e separado da marca de content-type pela **ultima**
    /// ocorrencia (uma pagina HTML pode conter o texto da marca).
    #[test]
    fn separa_corpo_do_content_type() {
        let marca = b"
__RAYCLIP_CT__";
        let mut saida = b"<html>__RAYCLIP_CT__ nao sou eu</html>".to_vec();
        saida.extend_from_slice(marca);
        saida.extend_from_slice(b"text/html; charset=utf-8");
        let i = rfind(&saida, marca).expect("marca presente");
        assert_eq!(&saida[..i], b"<html>__RAYCLIP_CT__ nao sou eu</html>");
        assert_eq!(
            String::from_utf8_lossy(&saida[i + marca.len()..]).trim(),
            "text/html; charset=utf-8"
        );
        assert!(rfind(b"sem marca", marca).is_none());
    }

    /// Classificacao de IP isolada (sem rede).
    #[test]
    fn classifica_ip() {
        use std::net::IpAddr;
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(ip_publico(ip("8.8.8.8")));
        assert!(ip_publico(ip("2606:4700:4700::1111")));
        assert!(!ip_publico(ip("127.0.0.1")));
        assert!(!ip_publico(ip("192.168.1.1")));
        assert!(!ip_publico(ip("172.31.255.255")));
        assert!(ip_publico(ip("172.32.0.1"))); // fora do /12: publico
        assert!(!ip_publico(ip("169.254.1.1")));
        assert!(!ip_publico(ip("::1")));
        assert!(!ip_publico(ip("fd12:3456::1")));
        assert!(!ip_publico(ip("224.0.0.1")));
        assert!(!ip_publico(ip("255.255.255.255")));
    }
}
