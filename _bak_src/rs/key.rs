//! Chave do banco.
//!
//! O SQLCipher recebe uma chave de 32 bytes; ela é guardada em `rayclip.key`
//! cifrada com DPAPI (`CryptProtectData`), então só o usuário do Windows que a
//! criou consegue abri-la — outro usuário da máquina (ou o arquivo copiado para
//! outro PC) não decifra nem a chave, muito menos o banco.
//!
//! Se o DPAPI falhar (perfil exótico/roaming), a chave fica protegida apenas
//! pelo ACL do arquivo: o banco continua cifrado, mas quem tiver acesso ao
//! arquivo da chave também abre o banco. O log avisa quando isso acontece.

use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, CryptProtectData, CryptUnprotectData, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

const KEY_FILE: &str = "rayclip.key";
const KEY_LEN: usize = 32;

/// Carrega (ou cria) a chave do banco em `dir`.
pub fn load_or_create(dir: &Path) -> Vec<u8> {
    let path = dir.join(KEY_FILE);
    if let Ok(blob) = std::fs::read(&path) {
        if let Some(key) = unprotect(&blob) {
            if key.len() == KEY_LEN {
                return key;
            }
        }
        // Nunca sobrescreve uma chave existente: se o DPAPI não abrir (outro
        // usuário, perfil diferente, backup restaurado), o banco fica fechado
        // em vez de ser trocado por um novo — nada é perdido no disco.
        crate::log("key: nao foi possivel abrir rayclip.key (DPAPI) - banco nao sera aberto");
        return random_bytes(KEY_LEN);
    }
    let key = random_bytes(KEY_LEN);
    let stored = protect(&key).unwrap_or_else(|| {
        crate::log("key: DPAPI indisponivel, gravando chave sem protecao do SO");
        key.clone()
    });
    if let Err(e) = std::fs::write(&path, &stored) {
        crate::log(&format!("key: falha ao gravar {e}"));
    }
    hide_file(&path);
    key
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

fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    let ok = unsafe {
        BCryptGenRandom(
            null_mut(),
            buf.as_mut_ptr(),
            n as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if ok != 0 {
        // Fallback improvável: mistura de tempo + endereço (não é CSPRNG).
        crate::log("key: BCryptGenRandom falhou, usando fallback");
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            ^ (&buf as *const _ as usize as u128);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((seed >> ((i % 16) * 8)) as u8)
                .wrapping_mul(31)
                .wrapping_add(i as u8);
        }
    }
    buf
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
