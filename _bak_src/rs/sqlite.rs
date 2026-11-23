//! Wrapper mínimo sobre o SQLCipher (compilado no `build.rs` com libtomcrypt).
//!
//! São só as funções que o app usa: abrir com chave, executar e consultar. O
//! banco é cifrado (AES-256 + HMAC-SHA512, formato SQLCipher 4) — o arquivo no
//! disco não tem nenhuma string legível.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

type DbHandle = *mut c_void;
type StmtHandle = *mut c_void;

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_OPEN_READWRITE: c_int = 0x0000_0002;
const SQLITE_OPEN_CREATE: c_int = 0x0000_0004;
/// `SQLITE_TRANSIENT`: o SQLite copia o valor na hora (não guardamos o ponteiro).
const SQLITE_TRANSIENT: isize = -1;
const COL_TEXT: c_int = 3;
const COL_BLOB: c_int = 4;

extern "C" {
    fn sqlite3_open_v2(
        path: *const c_char,
        db: *mut DbHandle,
        flags: c_int,
        vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close_v2(db: DbHandle) -> c_int;
    fn sqlite3_key(db: DbHandle, key: *const c_void, len: c_int) -> c_int;
    fn sqlite3_exec(
        db: DbHandle,
        sql: *const c_char,
        cb: *const c_void,
        arg: *mut c_void,
        err: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_errmsg(db: DbHandle) -> *const c_char;
    fn sqlite3_prepare_v2(
        db: DbHandle,
        sql: *const c_char,
        n: c_int,
        stmt: *mut StmtHandle,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_step(stmt: StmtHandle) -> c_int;
    fn sqlite3_reset(stmt: StmtHandle) -> c_int;
    fn sqlite3_finalize(stmt: StmtHandle) -> c_int;
    fn sqlite3_bind_text(stmt: StmtHandle, i: c_int, v: *const c_char, n: c_int, d: isize)
        -> c_int;
    fn sqlite3_bind_int64(stmt: StmtHandle, i: c_int, v: i64) -> c_int;
    fn sqlite3_bind_null(stmt: StmtHandle, i: c_int) -> c_int;
    fn sqlite3_bind_blob(stmt: StmtHandle, i: c_int, v: *const c_void, n: c_int, d: isize)
        -> c_int;
    fn sqlite3_column_type(stmt: StmtHandle, i: c_int) -> c_int;
    fn sqlite3_column_int64(stmt: StmtHandle, i: c_int) -> i64;
    fn sqlite3_column_text(stmt: StmtHandle, i: c_int) -> *const u8;
    fn sqlite3_column_bytes(stmt: StmtHandle, i: c_int) -> c_int;
    fn sqlite3_column_blob(stmt: StmtHandle, i: c_int) -> *const c_void;
    fn sqlite3_busy_timeout(db: DbHandle, ms: c_int) -> c_int;
    fn sqlite3_changes(db: DbHandle) -> c_int;
}

/// Um valor ligado/extraído de uma consulta.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Int(i64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<&str> for Val {
    fn from(v: &str) -> Self {
        Val::Text(v.to_string())
    }
}
impl From<String> for Val {
    fn from(v: String) -> Self {
        Val::Text(v)
    }
}
impl From<bool> for Val {
    fn from(v: bool) -> Self {
        Val::Int(i64::from(v))
    }
}
impl From<Option<String>> for Val {
    fn from(v: Option<String>) -> Self {
        match v {
            Some(s) => Val::Text(s),
            None => Val::Null,
        }
    }
}
impl From<Vec<u8>> for Val {
    fn from(v: Vec<u8>) -> Self {
        Val::Blob(v)
    }
}
impl<'a> From<&'a Vec<u8>> for Val {
    fn from(v: &'a Vec<u8>) -> Self {
        Val::Blob(v.clone())
    }
}

impl From<i64> for Val {
    fn from(v: i64) -> Self {
        Val::Int(v)
    }
}
impl From<Option<i64>> for Val {
    fn from(v: Option<i64>) -> Self {
        match v {
            Some(i) => Val::Int(i),
            None => Val::Null,
        }
    }
}
impl<'a> From<&'a [u8]> for Val {
    fn from(v: &'a [u8]) -> Self {
        Val::Blob(v.to_vec())
    }
}

impl Val {
    pub fn text(&self) -> Option<String> {
        match self {
            Val::Text(s) => Some(s.clone()),
            Val::Null => None,
            Val::Int(i) => Some(i.to_string()),
            Val::Blob(b) => Some(String::from_utf8_lossy(b).to_string()),
        }
    }
    pub fn int(&self) -> i64 {
        match self {
            Val::Int(i) => *i,
            Val::Text(s) => s.parse().unwrap_or(0),
            Val::Blob(b) => String::from_utf8_lossy(b).parse().unwrap_or(0),
            Val::Null => 0,
        }
    }
    pub fn blob(&self) -> Option<Vec<u8>> {
        match self {
            Val::Blob(b) => Some(b.clone()),
            _ => None,
        }
    }
}

/// Conexão aberta. `Clone` compartilha o mesmo handle (o chamador serializa).
#[derive(Clone)]
pub struct Db {
    inner: std::sync::Arc<Inner>,
}

struct Inner {
    h: DbHandle,
}

// O handle do SQLite pode ser usado de outra thread desde que serializado (o
// `Mutex` do chamador garante isso).
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Drop for Inner {
    fn drop(&mut self) {
        if !self.h.is_null() {
            unsafe {
                sqlite3_close_v2(self.h);
            }
        }
    }
}

impl Db {
    /// Abre (ou cria) o banco pedindo a chave. A chave nunca vai para o disco:
    /// o SQLCipher a usa para derivar a chave de página (com salt do arquivo).
    pub fn open(path: &Path, key: &[u8]) -> Result<Db, String> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "caminho inválido".to_string())?;
        let mut handle: DbHandle = std::ptr::null_mut();
        let rc = unsafe {
            sqlite3_open_v2(
                c_path.as_ptr(),
                &mut handle,
                SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
                std::ptr::null(),
            )
        };
        if rc != SQLITE_OK {
            let msg = if handle.is_null() {
                format!("sqlite3_open_v2 rc={rc}")
            } else {
                err_text(handle)
            };
            if !handle.is_null() {
                unsafe {
                    sqlite3_close_v2(handle);
                }
            }
            return Err(msg);
        }
        unsafe {
            let rc = sqlite3_key(handle, key.as_ptr().cast(), key.len() as c_int);
            if rc != SQLITE_OK {
                let msg = err_text(handle);
                sqlite3_close_v2(handle);
                return Err(format!("chave recusada: {msg}"));
            }
            sqlite3_busy_timeout(handle, 2000);
        }
        let db = Db {
            inner: std::sync::Arc::new(Inner { h: handle }),
        };
        // Falha aqui = chave errada (SQLITE_NOTADB) ou arquivo corrompido.
        db.exec("SELECT count(*) FROM sqlite_master")?;
        Ok(db)
    }

    /// Executa um ou mais comandos sem parâmetros.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
        let c_sql = CString::new(sql).map_err(|_| "sql com byte nulo".to_string())?;
        let mut err: *mut c_char = std::ptr::null_mut();
        let rc = unsafe {
            sqlite3_exec(
                self.inner.h,
                c_sql.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut err,
            )
        };
        if rc != SQLITE_OK {
            let msg = if err.is_null() {
                err_text(self.inner.h)
            } else {
                let text = unsafe { CStr::from_ptr(err) }.to_string_lossy().to_string();
                unsafe { free_sqlite(err) };
                text
            };
            return Err(msg);
        }
        Ok(())
    }

    /// Executa `INSERT/UPDATE/DELETE` e devolve quantas linhas mudaram.
    pub fn run(&self, sql: &str, params: &[Val]) -> Result<usize, String> {
        let stmt = self.prepare(sql, params)?;
        let rc = unsafe { sqlite3_step(stmt.h) };
        let changes = unsafe { sqlite3_changes(self.inner.h) } as usize;
        let err = if rc == SQLITE_DONE || rc == SQLITE_ROW {
            None
        } else {
            Some(err_text(self.inner.h))
        };
        drop(stmt);
        match err {
            Some(e) => Err(e),
            None => Ok(changes),
        }
    }

    /// Consulta e devolve todas as linhas.
    pub fn query(&self, sql: &str, params: &[Val]) -> Result<Vec<Vec<Val>>, String> {
        let stmt = self.prepare(sql, params)?;
        let mut rows = Vec::new();
        loop {
            let rc = unsafe { sqlite3_step(stmt.h) };
            if rc == SQLITE_ROW {
                let n = stmt.columns();
                let mut row = Vec::with_capacity(n);
                for i in 0..n {
                    row.push(stmt.column(i as c_int));
                }
                rows.push(row);
            } else if rc == SQLITE_DONE {
                break;
            } else {
                return Err(err_text(self.inner.h));
            }
        }
        Ok(rows)
    }

    pub fn query_opt(&self, sql: &str, params: &[Val]) -> Result<Option<Vec<Val>>, String> {
        Ok(self.query(sql, params)?.into_iter().next())
    }

    fn prepare(&self, sql: &str, params: &[Val]) -> Result<Stmt, String> {
        let c_sql = CString::new(sql).map_err(|_| "sql com byte nulo".to_string())?;
        let mut handle: StmtHandle = std::ptr::null_mut();
        let rc = unsafe {
            sqlite3_prepare_v2(
                self.inner.h,
                c_sql.as_ptr(),
                -1,
                &mut handle,
                std::ptr::null_mut(),
            )
        };
        if rc != SQLITE_OK {
            return Err(format!("{} ({sql})", err_text(self.inner.h)));
        }
        let stmt = Stmt { h: handle };
        for (idx, p) in params.iter().enumerate() {
            let i = (idx + 1) as c_int;
            let rc = unsafe {
                match p {
                    Val::Null => sqlite3_bind_null(handle, i),
                    Val::Int(v) => sqlite3_bind_int64(handle, i, *v),
                    Val::Text(s) => sqlite3_bind_text(
                        handle,
                        i,
                        s.as_ptr().cast(),
                        s.len() as c_int,
                        SQLITE_TRANSIENT,
                    ),
                    Val::Blob(b) => sqlite3_bind_blob(
                        handle,
                        i,
                        b.as_ptr().cast(),
                        b.len() as c_int,
                        SQLITE_TRANSIENT,
                    ),
                }
            };
            if rc != SQLITE_OK {
                return Err(format!("bind {i}: {}", err_text(self.inner.h)));
            }
        }
        Ok(stmt)
    }
}

struct Stmt {
    h: StmtHandle,
}

impl Stmt {
    fn column(&self, i: c_int) -> Val {
        unsafe {
            match sqlite3_column_type(self.h, i) {
                COL_TEXT => {
                    let p = sqlite3_column_text(self.h, i);
                    let n = sqlite3_column_bytes(self.h, i).max(0) as usize;
                    if p.is_null() {
                        Val::Null
                    } else {
                        Val::Text(String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into())
                    }
                }
                COL_BLOB => {
                    let p = sqlite3_column_blob(self.h, i);
                    let n = sqlite3_column_bytes(self.h, i).max(0) as usize;
                    if p.is_null() {
                        Val::Blob(Vec::new())
                    } else {
                        Val::Blob(std::slice::from_raw_parts(p.cast::<u8>(), n).to_vec())
                    }
                }
                5 => Val::Null, // SQLITE_NULL
                _ => Val::Int(sqlite3_column_int64(self.h, i)),
            }
        }
    }

    fn columns(&self) -> usize {
        // `sqlite3_column_count` não está exposta: consultamos 1 a 1 até NULL
        // não serviria, então usamos a API certa.
        unsafe { column_count(self.h) as usize }
    }
}

extern "C" {
    #[link_name = "sqlite3_column_count"]
    fn column_count(stmt: StmtHandle) -> c_int;
    #[link_name = "sqlite3_free"]
    fn free_sqlite(p: *mut c_char);
}

impl Drop for Stmt {
    fn drop(&mut self) {
        unsafe {
            sqlite3_reset(self.h);
            sqlite3_finalize(self.h);
        }
    }
}

fn err_text(db: DbHandle) -> String {
    unsafe {
        let p = sqlite3_errmsg(db);
        if p.is_null() {
            "erro desconhecido do SQLite".to_string()
        } else {
            CStr::from_ptr(p).to_string_lossy().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escreve_cifrado_e_abre_com_a_chave() {
        let dir = std::env::temp_dir().join(format!("rayclip_sqltest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        let key = [7u8; 32];
        {
            let db = Db::open(&path, &key).unwrap();
            db.exec("CREATE TABLE t(x TEXT, n INTEGER);").unwrap();
            db.run(
                "INSERT INTO t VALUES(?1, ?2)",
                &["SEGREDO-123".into(), 42i64.into()],
            )
            .unwrap();
            let rows = db.query("SELECT x, n FROM t", &[]).unwrap();
            assert_eq!(rows[0][0], Val::Text("SEGREDO-123".into()));
            assert_eq!(rows[0][1].int(), 42);
        }
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(11).any(|w| w == b"SEGREDO-123"));
        assert!(!raw.windows(9).any(|w| w == b"SQLite fo"));
        // Chave errada: precisa falhar.
        assert!(Db::open(&path, &[9u8; 32]).is_err());
        // Chave certa: lê de volta.
        let db = Db::open(&path, &key).unwrap();
        let rows = db.query("SELECT x FROM t", &[]).unwrap();
        assert_eq!(rows[0][0].text().unwrap(), "SEGREDO-123");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blob_ida_e_volta() {
        let dir = std::env::temp_dir().join(format!("rayclip_sqlblob_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("b.db"), &[1u8; 32]).unwrap();
        db.exec("CREATE TABLE b(id TEXT PRIMARY KEY, data BLOB);")
            .unwrap();
        let bytes = vec![0u8, 159, 146, 150, 255, 7];
        db.run(
            "INSERT INTO b VALUES(?1, ?2)",
            &["x".into(), bytes.clone().into()],
        )
        .unwrap();
        let row = db
            .query_opt("SELECT data FROM b WHERE id = ?1", &["x".into()])
            .unwrap();
        assert_eq!(row.unwrap()[0].blob().unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
