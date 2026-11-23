//! Chave do banco.
//!
//! O SQLCipher recebe uma chave de 32 bytes; ela é guardada em `rayclip.key`
//! cifrada com DPAPI (`CryptProtectData`), então só o usuário do Windows que a
//! criou consegue abri-la — outro usuário da máquina (ou o arquivo copiado para
//! outro PC) não decifra nem a chave, muito menos o banco.
//!
//! **Política fail-closed (v0.1.26).** Não existe mais chave "de reserva":
//!
//! * sem `BCryptGenRandom`, não se inventa uma chave a partir de tempo e
//!   endereço de memória (isso não é CSPRNG e deixaria o banco previsível) —
//!   a função devolve `None` e o banco simplesmente não abre;
//! * sem DPAPI, não se grava a chave crua no arquivo: `None` também. É melhor
//!   o histórico não abrir do que abrir com a chave legível ao lado do banco;
//! * o app continua rodando (só em memória, `Store::degraded`), para o usuário
//!   ver a explicação em Configurações em vez de um app que não abre;
//! * uma chave **antiga** gravada sem DPAPI (builds pré-0.1.26) é aceita e
//!   re-protegida na hora — não se joga fora o histórico de ninguém.

use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, CryptProtectData, CryptUnprotectData, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

const KEY_FILE: &str = "rayclip.key";
const KEY_LEN: usize = 32;

/// Resultado de abrir/criar a chave: a chave, ou o motivo de não haver chave.
pub type Chave = Result<Vec<u8>, String>;

/// Carrega (ou cria) a chave do banco em `dir`.
///
/// `Err(motivo)` significa "não dá para abrir o banco com segurança" — o
/// chamador segue em memória e mostra o motivo na interface.
pub fn load_or_create(dir: &Path) -> Chave {
    let path = dir.join(KEY_FILE);
    if let Ok(blob) = std::fs::read(&path) {
        if let Some(key) = unprotect(&blob) {
            if key.len() == KEY_LEN {
                return Ok(key);
            }
            crate::log("key: rayclip.key abriu mas o tamanho nao e 32 bytes");
            return Err("the key file is damaged (wrong length)".into());
        }
        // Chave de uma build antiga gravada **sem** DPAPI (quando o DPAPI
        // falhava o código antigo escrevia os 32 bytes crus). Aceita — senão o
        // histórico existente ficaria inacessível para sempre — e re-protegida
        // agora, para o arquivo deixar de ter a chave em claro.
        if blob.len() == KEY_LEN {
            crate::log("key: chave antiga sem DPAPI - re-protegendo");
            match protect(&blob) {
                Some(stored) if std::fs::write(&path, &stored).is_ok() => {
                    hide_file(&path);
                    return Ok(blob);
                }
                _ => {
                    return Err(
                        "the key file is stored without Windows protection and could not be re-protected"
                            .into(),
                    )
                }
            }
        }
        // Nunca sobrescreve uma chave existente: se o DPAPI não abrir (outro
        // usuário, perfil diferente, backup restaurado), o banco fica fechado
        // em vez de ser trocado por um novo — nada é perdido no disco.
        crate::log("key: nao foi possivel abrir rayclip.key (DPAPI) - banco nao sera aberto");
        return Err(
            "the key file belongs to another Windows user or profile (DPAPI could not open it)"
                .into(),
        );
    }

    // Arquivo novo: exige CSPRNG e DPAPI. Sem qualquer um dos dois, não grava.
    let Some(key) = random_bytes(KEY_LEN) else {
        crate::log("key: BCryptGenRandom falhou - sem chave, banco nao sera criado");
        return Err("the system random number generator failed".into());
    };
    let Some(stored) = protect(&key) else {
        crate::log("key: DPAPI indisponivel - sem chave, banco nao sera criado");
        return Err("Windows DPAPI is not available for this user".into());
    };
    if let Err(e) = std::fs::write(&path, &stored) {
        crate::log(&format!("key: falha ao gravar {e}"));
        return Err(format!("could not write the key file ({e})"));
    }
    hide_file(&path);
    Ok(key)
}

fn blob_of(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    }
}

fn take_blob(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = if out.pbData.is_null() || out.cbData == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() }
    };
    if !out.pbData.is_null() {
        unsafe {
            LocalFree(out.pbData as *mut std::ffi::c_void);
        }
    }
    bytes
}

fn protect(key: &[u8]) -> Option<Vec<u8>> {
    let input = blob_of(key);
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return None;
    }
    Some(take_blob(output))
}

fn unprotect(blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < 16 {
        return None;
    }
    let input = blob_of(blob);
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return None;
    }
    Some(take_blob(output))
}

/// 32 bytes do CSPRNG do sistema. `None` quando a API falha — sem alternativa
/// "criativa": chave previsível não é chave.
fn random_bytes(n: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; n];
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            buf.as_mut_ptr(),
            n as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return None;
    }
    Some(buf)
}

/// Deixa o arquivo da chave oculto (não é segurança, é higiene).
fn hide_file(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    unsafe {
        SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um blob que não é DPAPI e não tem 32 bytes nunca vira chave.
    #[test]
    fn blob_estranho_nao_vira_chave() {
        assert!(unprotect(b"curto").is_none());
        assert!(unprotect(&[0u8; 64]).is_none());
    }

    /// A chave gerada é aleatória de verdade (duas chamadas não coincidem).
    #[test]
    fn chave_nova_sai_do_csprng() {
        let a = random_bytes(KEY_LEN).expect("BCryptGenRandom no Windows");
        let b = random_bytes(KEY_LEN).expect("BCryptGenRandom no Windows");
        assert_eq!(a.len(), KEY_LEN);
        assert_ne!(a, b);
    }

    /// Chave crua antiga (pré-0.1.26) é aceita e sai do disco protegida.
    #[test]
    fn chave_antiga_sem_dpapi_e_reprotegida() {
        let dir = std::env::temp_dir().join(format!("rayclip_key_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(KEY_FILE);
        let crua = random_bytes(KEY_LEN).unwrap();
        std::fs::write(&path, &crua).unwrap();

        let chave = load_or_create(&dir).expect("deveria aceitar a chave antiga");
        assert_eq!(chave, crua);

        let gravado = std::fs::read(&path).unwrap();
        assert_ne!(gravado, crua, "a chave crua deveria ter sido substituida");
        assert_eq!(unprotect(&gravado).unwrap(), crua, "DPAPI deveria abrir a nova");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Chave ilegível (outro usuário) não é sobrescrita nem substituída.
    #[test]
    fn chave_ilegivel_nao_e_sobrescrita() {
        let dir = std::env::temp_dir().join(format!("rayclip_key_bad_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(KEY_FILE);
        let lixo = vec![7u8; 64];
        std::fs::write(&path, &lixo).unwrap();

        assert!(load_or_create(&dir).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), lixo, "arquivo intacto");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
