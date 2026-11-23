import io

f = "src-tauri/src/linkmeta.rs"
s = io.open(f, encoding="utf-8").read()

ANTIGO_HTTP_GET = '''fn http_get(url: &str, max: usize) -> Option<(String, Vec<u8>)> {
    use std::io::Read;
    let resp = agent().get(url).call().ok()?;
    let ctype = resp.header("content-type").unwrap_or("").to_string();'''

NOVO_HTTP_GET = '''/// Destino aceitavel para uma requisicao automatica?
///
/// O Rayclip busca metadados de qualquer link copiado - inclusive um link que
/// outra pessoa escreveu. Sem esta barreira, copiar
/// `http://192.168.0.1/admin` (ou `http://localhost:8080`) faria o app bater
/// dentro da rede local: o classico SSRF. So passa **http/https** para um host
/// que resolva **exclusivamente** para enderecos publicos.
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
    let ctype = resp.header("content-type").unwrap_or("").to_string();'''

ANTIGO_HEAD = '''fn head_ct(url: &str) -> Option<String> {
    let resp = agent().head(url).call().ok()?;
    Some(resp.header("content-type").unwrap_or("").to_string())
}

fn http_get_final(url: &str, max: usize) -> Option<(String, String, Vec<u8>)> {
    use std::io::Read;
    let resp = agent().get(url).call().ok()?;
    let ctype = resp.header("content-type").unwrap_or("").to_string();
    let final_url = resp.get_url().to_string();'''

NOVO_HEAD = '''fn head_ct(url: &str) -> Option<String> {
    let (_, resp) = pedir(url, true, false)?;
    Some(resp.header("content-type").unwrap_or("").to_string())
}

fn http_get_final(url: &str, max: usize) -> Option<(String, String, Vec<u8>)> {
    use std::io::Read;
    let (final_url, resp) = pedir(url, false, false)?;
    let ctype = resp.header("content-type").unwrap_or("").to_string();'''

ANTIGO_GET_HTML = '''fn get_html(url: &str) -> Option<ureq::Response> {
    agent()
        .get(url)
        .set(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .set("Accept-Language", "pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7")
        .set("Upgrade-Insecure-Requests", "1")
        .call()
        .ok()
}'''

NOVO_GET_HTML = '''fn get_html(url: &str) -> Option<ureq::Response> {
    pedir(url, false, true).map(|(_, r)| r)
}'''

ANTIGO_AGENT = '''    ureq::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(8))
        .build()'''

NOVO_AGENT = '''    ureq::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(8))
        // Sem redirecionamento automatico: `pedir()` segue os saltos um a um,
        // validando o destino de cada um (anti-SSRF).
        .redirects(0)
        .build()'''

ANTIGO_CURL = '''    let out = Command::new("curl.exe")
        .args([
            "-sS",
            "-L",
            "--max-time",'''

NOVO_CURL = '''    // O curl e um processo separado: nao da para ver os saltos de um `-L`, entao
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
            "--max-time",'''

for antigo, novo in [
    (ANTIGO_HTTP_GET, NOVO_HTTP_GET),
    (ANTIGO_HEAD, NOVO_HEAD),
    (ANTIGO_GET_HTML, NOVO_GET_HTML),
    (ANTIGO_AGENT, NOVO_AGENT),
    (ANTIGO_CURL, NOVO_CURL),
]:
    assert antigo in s, antigo[:60]
    s = s.replace(antigo, novo, 1)

io.open(f, "w", encoding="utf-8", newline="").write(s)
print("linkmeta ok")
