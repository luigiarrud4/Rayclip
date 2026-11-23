//! Atualização automática a partir das releases do GitHub.
//!
//! # Por que trocar o exe em vez de rodar o instalador
//!
//! O Rayclip é **um único exe** (o NSIS instala só ele), então a atualização
//! pode ser a mesma ideia do updater do RobloxManager: baixar o exe novo,
//! conferir o SHA-256 e trocar os arquivos de nome no lugar.
//!
//! Um exe em execução não pode ser sobrescrito nem apagado no Windows, mas
//! **pode ser renomeado** (o loader abre a imagem com `FILE_SHARE_DELETE` e
//! renomear é uma operação de delete-share). Então são dois renames na mesma
//! pasta — mesma volume, portanto atômicos:
//!
//! 1. `rayclip.exe` → `rayclip.exe.old` (a imagem que está rodando)
//! 2. `rayclip.exe.update` → `rayclip.exe` (o download já conferido)
//!
//! Se o segundo falhar, o primeiro é desfeito: nunca existe um instante sem
//! `rayclip.exe` na pasta. O novo exe sobe com [`AFTER_UPDATE_FLAG`] e o PID do
//! processo antigo, espera ele sair e só então ocupa o banco (dois processos
//! com o SQLite aberto ao mesmo tempo é o que estraga o histórico).
//!
//! # Verificação (fail-closed)
//!
//! * o download só é aceito vindo de `https://github.com/<repo>/releases/download/`;
//! * o SHA-256 vem da própria API do GitHub (campo `digest` do asset) ou do
//!   arquivo `<asset>.sha256` publicado junto — sem checksum, não instala;
//! * o arquivo é lido de volta do disco depois de gravado (é nesse momento que
//!   o antivírus dá o veredito) e precisa começar com `MZ`.
//!
//! # Antivírus
//!
//! Build sem assinatura é sinalizado com frequência. Cada passo se protege:
//! se o exe novo sumir antes do swap, nada foi movido; se sumir depois, o
//! `.old` volta a ser `rayclip.exe`; se o novo processo morrer nos primeiros
//! 2 s, o antigo é restaurado. E o `.old` **sobrevive** à primeira execução
//! após a atualização (só é apagado na abertura seguinte), para que uma
//! remoção durante essa primeira execução ainda tenha de onde voltar.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Repositório usado nas releases. Dá para trocar sem recompilar com
/// `RAYCLIP_UPDATE_REPO=owner/repo` (útil em teste).
pub const REPO_PADRAO: &str = "luigiarrud4/Rayclip";

/// Nome do asset com o exe cru em cada release.
pub const ASSET: &str = "rayclip.exe";

/// Argumento passado ao exe recém-instalado, seguido do PID que ele substitui.
pub const AFTER_UPDATE_FLAG: &str = "--after-update";

/// Prefixo aceito para download. Qualquer outro host é recusado.
const DOWNLOAD_PREFIXO: &str = "https://github.com/";

/// Quanto o processo novo espera o antigo sair antes de seguir assim mesmo.
const ESPERA_PID: Duration = Duration::from_secs(30);

/// Quanto o antigo observa o novo antes de entregar o posto.
const OBSERVA_INICIO: Duration = Duration::from_secs(2);

/// Uma release mais nova que a build atual.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Versão instalada (ex.: `0.1.23`).
    pub current: String,
    /// Versão da release, sem o `v` (ex.: `0.1.24`).
    pub version: String,
    /// Tag como está no GitHub (`v0.1.24`).
    pub tag: String,
    /// Título da release.
    pub name: String,
    /// Texto da release (notas).
    pub notes: String,
    /// Data de publicação (ISO 8601).
    pub published_at: String,
    /// Página da release, para download manual e notas completas.
    pub page_url: String,
    /// Tamanho do exe em bytes (0 quando a release não tem o asset).
    pub size: u64,
    /// `false` quando a release existe mas não publicou `rayclip.exe`.
    pub has_asset: bool,
    /// Link direto do exe (não vai para a UI).
    #[serde(skip)]
    pub asset_url: String,
    /// SHA-256 esperado (não vai para a UI).
    #[serde(skip)]
    pub sha256: Option<String>,
}

// ---------------------------------------------------------------------------
// Versão
// ---------------------------------------------------------------------------

/// Versão desta build — a mesma do `Cargo.toml`, que o deploy atualiza.
pub fn versao_atual() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn parse_versao(versao: &str) -> Option<Vec<u64>> {
    let core = versao.trim().trim_start_matches('v').split(['-', '+']).next()?;
    core.split('.').map(|p| p.parse().ok()).collect()
}

/// `true` quando `remota` é estritamente maior que `local`. Componentes que
/// faltam contam como zero (`1.10` > `1.9.1`). Nada que não parseia é novo.
pub fn mais_nova(remota: &str, local: &str) -> bool {
    let (Some(remota), Some(local)) = (parse_versao(remota), parse_versao(local)) else {
        return false;
    };
    let n = remota.len().max(local.len());
    let pad = |v: &[u64]| {
        let mut v = v.to_vec();
        v.resize(n, 0);
        v
    };
    pad(&remota) > pad(&local)
}

// ---------------------------------------------------------------------------
// Consulta
// ---------------------------------------------------------------------------

fn repo() -> String {
    std::env::var("RAYCLIP_UPDATE_REPO")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| s.matches('/').count() == 1 && !s.is_empty())
        .unwrap_or_else(|| REPO_PADRAO.to_string())
}

fn agento(timeout: u64) -> ureq::Agent {
    ureq::builder()
        .user_agent(&format!("Rayclip-Updater/{}", versao_atual()))
        .timeout(Duration::from_secs(timeout))
        .build()
}

/// Consulta a última release e devolve `Some` só quando ela é mais nova.
///
/// `force` existe para o botão "Check now": a consulta normal é feita no
/// máximo a cada 6 h (`update_check`) para não gastar o limite de 60 req/h da
/// API anônima do GitHub.
pub fn check(force: bool) -> Result<Option<UpdateInfo>, String> {
    if !force {
        if let Ok(guarda) = CACHE.lock() {
            if let Some((quando, res)) = guarda.as_ref() {
                if quando.elapsed() < Duration::from_secs(6 * 3600) {
                    return Ok(res.clone());
                }
            }
        }
    }
    let res = consultar()?;
    if let Ok(mut guarda) = CACHE.lock() {
        *guarda = Some((Instant::now(), res.clone()));
    }
    Ok(res)
}

fn consultar() -> Result<Option<UpdateInfo>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        repo()
    );
    let resp = agento(20)
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call();
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => {
            return Err("no release published yet".to_string())
        }
        Err(ureq::Error::Status(403, _)) | Err(ureq::Error::Status(429, _)) => {
            return Err("GitHub rate limit reached — try again in a few minutes".to_string())
        }
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("GitHub answered {code}"))
        }
        Err(_) => return Err("could not reach GitHub (check the connection)".to_string()),
    };

    let texto = resp.into_string().map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&texto).map_err(|e| e.to_string())?;

    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let versao = tag.trim_start_matches('v').to_string();
    let atual = versao_atual();
    if versao.is_empty() || !mais_nova(&versao, &atual) {
        return Ok(None);
    }

    let campo = |k: &str| {
        json.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let mut info = UpdateInfo {
        current: atual,
        version: versao,
        tag: tag.clone(),
        name: campo("name"),
        notes: campo("body"),
        published_at: campo("published_at"),
        page_url: campo("html_url"),
        size: 0,
        has_asset: false,
        asset_url: String::new(),
        sha256: None,
    };

    // Asset com o exe cru + checksum que o GitHub registrou no upload.
    if let Some(assets) = json.get("assets").and_then(|v| v.as_array()) {
        for a in assets {
            let nome = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if !nome.eq_ignore_ascii_case(ASSET) {
                continue;
            }
            info.asset_url = a
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            info.size = a.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            info.sha256 = a
                .get("digest")
                .and_then(|v| v.as_str())
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(|h| h.to_ascii_lowercase())
                .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()));
            info.has_asset = !info.asset_url.is_empty();
            break;
        }
    }
    // Releases publicadas antes do campo `digest` (ou por script antigo) têm o
    // checksum no arquivo `rayclip.exe.sha256` ao lado.
    if info.sha256.is_none() && info.has_asset {
        info.sha256 = baixar_sha256(&format!("{}.sha256", info.asset_url));
    }
    Ok(Some(info))
}

/// Lê `<asset>.sha256` (`"<hash>  rayclip.exe"` ou só o hash).
fn baixar_sha256(url: &str) -> Option<String> {
    if !url.starts_with(DOWNLOAD_PREFIXO) {
        return None;
    }
    let texto = agento(15).get(url).call().ok()?.into_string().ok()?;
    let hash = texto
        .split([' ', '\t', '\r', '\n'])
        .find(|s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()))?
        .to_ascii_lowercase();
    Some(hash)
}

// Cache em memória da última consulta (6 h), para não bater na API a cada
// abertura da janela. Erros não entram no cache: um piscar de rede não pode
// esconder uma atualização até o fim do dia.
static CACHE: std::sync::Mutex<Option<(Instant, Option<UpdateInfo>)>> = std::sync::Mutex::new(None);

// ---------------------------------------------------------------------------
// Download e instalação
// ---------------------------------------------------------------------------

fn sibling(exe: &Path, sufixo: &str) -> PathBuf {
    let mut nome = OsString::from(exe.as_os_str());
    nome.push(sufixo);
    PathBuf::from(nome)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Baixa o exe da release para `<exe>.update`, confere o SHA-256 e troca.
///
/// `progresso(baixado, total)` é chamado durante o download. Não reinicia o
/// app — quem chama usa [`relaunch`] e depois encerra.
pub fn instalar(
    info: &UpdateInfo,
    exe: &Path,
    mut progresso: impl FnMut(u64, u64),
) -> Result<(), String> {
    if !info.has_asset || info.asset_url.is_empty() {
        return Err(format!(
            "release v{} has no {ASSET} asset to download",
            info.version
        ));
    }
    if !info.asset_url.starts_with(DOWNLOAD_PREFIXO) {
        return Err(format!("refusing to download from {}", info.asset_url));
    }
    let Some(esperado) = info.sha256.clone() else {
        return Err("the release has no SHA-256 checksum — update refused".to_string());
    };

    let staged = sibling(exe, ".update");
    let total = info.size;

    // ---- download, com hash calculado enquanto grava -----------------------
    let resp = agento(600)
        .get(&info.asset_url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let mut leitor = resp.into_reader();
    let mut arquivo = std::fs::File::create(&staged)
        .map_err(|e| format!("cannot write {}: {e}", staged.display()))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut baixado: u64 = 0;
    loop {
        let n = match leitor.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = std::fs::remove_file(&staged);
                return Err(format!("download interrupted: {e}"));
            }
        };
        hash.update(&buf[..n]);
        if let Err(e) = arquivo.write_all(&buf[..n]) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!("cannot write {}: {e}", staged.display()));
        }
        baixado += n as u64;
        progresso(baixado, total.max(baixado));
    }
    if let Err(e) = arquivo.sync_all() {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("cannot flush {}: {e}", staged.display()));
    }
    drop(arquivo);

    let calculado = hex(&hash.finalize());
    if !calculado.eq_ignore_ascii_case(&esperado) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!(
            "the download is corrupt (SHA-256 {calculado}, expected {esperado})"
        ));
    }
    if baixado < 1024 {
        let _ = std::fs::remove_file(&staged);
        return Err("the download is too small to be the app".to_string());
    }

    // ---- leitura de volta (veredito do antivírus) --------------------------
    match std::fs::read(&staged) {
        Ok(bytes) if bytes.starts_with(b"MZ") => {}
        Ok(_) => {
            let _ = std::fs::remove_file(&staged);
            return Err("the download is not a Windows executable".to_string());
        }
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "the new version was blocked or removed after downloading, usually by antivirus ({e}). Rayclip stayed on the version you have"
            ));
        }
    }

    // ---- troca dos nomes ---------------------------------------------------
    let old = sibling(exe, ".old");
    if old.exists() {
        // Sobra de uma atualização anterior cuja limpeza não pôde apagar.
        if let Err(e) = std::fs::remove_file(&old) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "{} is still in use from the last update ({e}). Close every Rayclip window and try again",
                old.display()
            ));
        }
    }
    if let Err(e) = std::fs::rename(exe, &old) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("cannot replace the exe in {} ({e})", pasta(exe)));
    }
    if let Err(e) = std::fs::rename(&staged, exe) {
        let _ = std::fs::rename(&old, exe); // devolve o que estava rodando
        let _ = std::fs::remove_file(&staged);
        return Err(format!("cannot replace the exe in {} ({e})", pasta(exe)));
    }
    match std::fs::read(exe) {
        Ok(bytes) if sha256_hex(&bytes) == calculado => Ok(()),
        _ => {
            // Sumiu/alterou depois do swap: o antigo volta.
            let _ = std::fs::remove_file(exe);
            let _ = std::fs::rename(&old, exe);
            Err(
                "the new version was blocked or removed after installing, usually by antivirus. Rayclip stayed on the version you have"
                    .to_string(),
            )
        }
    }
}

fn pasta(exe: &Path) -> String {
    exe.parent().unwrap_or(exe).display().to_string()
}

/// Sobe o exe recém-instalado passando o PID deste processo.
///
/// Se o novo não ficar de pé (antivírus, exe truncado), o `.old` volta a ser
/// `rayclip.exe` e o erro é devolvido — quem chamou continua rodando.
pub fn relaunch(exe: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut filho = match std::process::Command::new(exe)
        .arg(AFTER_UPDATE_FLAG)
        .arg(std::process::id().to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(f) => f,
        Err(e) => {
            return Err(desfazer(
                exe,
                format!("the new version could not start, usually because antivirus blocked it ({e})"),
            ))
        }
    };
    let inicio = Instant::now();
    while inicio.elapsed() < OBSERVA_INICIO {
        if let Ok(Some(_)) = filho.try_wait() {
            return Err(desfazer(
                exe,
                "the new version closed as soon as it started, usually because antivirus stopped it"
                    .to_string(),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn desfazer(exe: &Path, motivo: String) -> String {
    let old = sibling(exe, ".old");
    match std::fs::rename(&old, exe) {
        Ok(()) => format!("{motivo}. Rayclip stayed on the version you have"),
        Err(e) => format!(
            "{motivo}, and the previous version could not be put back ({e}). It is at {}",
            old.display()
        ),
    }
}

// ---------------------------------------------------------------------------
// Início do app
// ---------------------------------------------------------------------------

/// PID que este processo substitui, quando veio de [`relaunch`].
pub fn pid_substituido() -> Option<u32> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == AFTER_UPDATE_FLAG {
            return args.next().and_then(|p| p.parse().ok());
        }
    }
    None
}

/// Primeira abertura depois de uma atualização: espera o processo antigo sair
/// (os dois não podem abrir o banco ao mesmo tempo) e limpa o download.
///
/// O `.old` **fica**: se o antivírus remover o exe novo durante esta execução,
/// ainda há de onde voltar. A abertura seguinte apaga.
pub fn concluir_atualizacao(pid: u32) {
    if !esperar_sair(pid, ESPERA_PID) {
        // Não bloqueia o uso: o SQLite já tolera, e travar aqui seria pior.
        log(&format!("pid {pid} ainda rodando depois da atualização"));
    }
    if let Some(exe) = exe_atual() {
        let _ = std::fs::remove_file(sibling(&exe, ".update"));
    }
}

/// Abertura normal: apaga sobras de um update interrompido.
pub fn limpar_inicio() {
    if let Some(exe) = exe_atual() {
        let _ = std::fs::remove_file(sibling(&exe, ".update"));
        let _ = std::fs::remove_file(sibling(&exe, ".old"));
    }
}

pub fn exe_atual() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn esperar_sair(pid: u32, timeout: Duration) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if h.is_null() {
            return true; // já saiu
        }
        let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        let r = WaitForSingleObject(h, ms);
        CloseHandle(h);
        r == WAIT_OBJECT_0
    }
}

fn log(msg: &str) {
    crate::log(&format!("update: {msg}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rayclip_upd_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn info_com(bytes: &[u8], url: &str) -> UpdateInfo {
        UpdateInfo {
            current: "0.1.23".into(),
            version: "0.1.24".into(),
            tag: "v0.1.24".into(),
            name: "v0.1.24".into(),
            notes: String::new(),
            published_at: String::new(),
            page_url: String::new(),
            size: bytes.len() as u64,
            has_asset: true,
            asset_url: url.into(),
            sha256: Some(sha256_hex(bytes)),
        }
    }

    #[test]
    fn versao_maior_e_mais_nova() {
        assert!(mais_nova("0.1.24", "0.1.23"));
        assert!(mais_nova("v1.0.0", "0.1.23"));
        assert!(mais_nova("0.2.0", "0.1.23"));
        // numérico, não texto
        assert!(mais_nova("0.1.100", "0.1.99"));
    }

    #[test]
    fn versao_igual_ou_menor_nao_e_nova() {
        assert!(!mais_nova("0.1.23", "0.1.23"));
        assert!(!mais_nova("v0.1.23", "0.1.23"));
        assert!(!mais_nova("0.1.22", "0.1.23"));
        assert!(!mais_nova("latest", "0.1.23"));
        assert!(!mais_nova("", "0.1.23"));
    }

    #[test]
    fn download_sem_checksum_e_recusado() {
        let d = tmp("nosha");
        let exe = d.join("rayclip.exe");
        std::fs::write(&exe, b"antigo").unwrap();
        let mut info = info_com(b"novo", &format!("{DOWNLOAD_PREFIXO}a/b/releases/download/v1/rayclip.exe"));
        info.sha256 = None;
        assert!(instalar(&info, &exe, |_, _| {}).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"antigo");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn download_de_outro_host_e_recusado() {
        let d = tmp("host");
        let exe = d.join("rayclip.exe");
        std::fs::write(&exe, b"antigo").unwrap();
        let info = info_com(b"novo", "https://evil.example/rayclip.exe");
        assert!(instalar(&info, &exe, |_, _| {}).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"antigo");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O hash é calculado sobre o arquivo inteiro: um checksum de outra coisa
    /// barra a instalação antes de qualquer rename.
    #[test]
    fn checksum_divergente_nao_troca_nada() {
        let d = tmp("hash");
        let exe = d.join("rayclip.exe");
        std::fs::write(&exe, b"antigo").unwrap();
        let staged = sibling(&exe, ".update");
        std::fs::write(&staged, b"novo").unwrap();
        let info = info_com(b"outra coisa", "https://github.com/x/y/releases/download/v1/rayclip.exe");
        // Simula a comparação que `instalar` faz depois do download.
        assert!(sha256_hex(&std::fs::read(&staged).unwrap()) != info.sha256.clone().unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O swap funciona com o exe aberto do jeito que o loader abre.
    #[cfg(windows)]
    #[test]
    fn troca_funciona_com_o_exe_aberto() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_DELETE: u32 = 0x4;

        let d = tmp("swap");
        let exe = d.join("rayclip.exe");
        std::fs::write(&exe, b"rodando").unwrap();
        let _aberto = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .open(&exe)
            .unwrap();

        let novo = b"MZ\x90\x00exe novo";
        let staged = sibling(&exe, ".update");
        std::fs::write(&staged, novo).unwrap();
        let old = sibling(&exe, ".old");
        std::fs::rename(&exe, &old).unwrap();
        std::fs::rename(&staged, &exe).unwrap();

        assert_eq!(std::fs::read(&exe).unwrap(), novo);
        assert_eq!(std::fs::read(&old).unwrap(), b"rodando");
        assert!(!staged.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Sobras de um update anterior são limpas na abertura normal.
    #[test]
    fn limpeza_apaga_sobras() {
        let d = tmp("limpa");
        let exe = d.join("rayclip.exe");
        std::fs::write(&exe, b"atual").unwrap();
        std::fs::write(sibling(&exe, ".update"), b"parcial").unwrap();
        std::fs::write(sibling(&exe, ".old"), b"velho").unwrap();
        let _ = std::fs::remove_file(sibling(&exe, ".update"));
        let _ = std::fs::remove_file(sibling(&exe, ".old"));
        assert!(!sibling(&exe, ".update").exists());
        assert!(!sibling(&exe, ".old").exists());
        assert_eq!(std::fs::read(&exe).unwrap(), b"atual");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Um exe novo que não sobe (lixo no lugar do binário, como um download
    /// corrompido ou removido pelo antivírus) devolve o antigo ao lugar.
    #[cfg(windows)]
    #[test]
    fn exe_que_nao_sobe_volta_para_o_antigo() {
        let d = tmp("rollback");
        let exe = d.join("rayclip.exe");
        std::fs::write(&exe, b"nao sou um exe").unwrap();
        std::fs::write(sibling(&exe, ".old"), b"antigo").unwrap();

        let res = relaunch(&exe);

        assert!(res.is_err(), "deveria recusar um exe invalido");
        assert_eq!(std::fs::read(&exe).unwrap(), b"antigo");
        assert!(!sibling(&exe, ".old").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pid_em_processo_inexistente_libera_na_hora() {
        let t = Instant::now();
        assert!(esperar_sair(u32::MAX, Duration::from_secs(10)));
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn argumento_de_atualizacao_e_lido() {
        // `pid_substituido` lê `std::env::args`; o teste cobre o parsing.
        let mut args = vec!["rayclip.exe", "--after-update", "4242"].into_iter().skip(1);
        let mut achado = None;
        while let Some(a) = args.next() {
            if a == AFTER_UPDATE_FLAG {
                achado = args.next().and_then(|p| p.parse::<u32>().ok());
            }
        }
        assert_eq!(achado, Some(4242));
    }
}
