//! Histórico em banco SQLCipher (`rayclip.db`), com as imagens em BLOB.
//!
//! Nada do clipboard fica em arquivo aberto: texto, HTML/RTF e as imagens
//! (bytes PNG originais) vão para o banco cifrado. O `store.json` e a pasta
//! `images/` do formato antigo são migrados na primeira abertura e apagados.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::sqlite::{Db, Val};

pub const MAX_ENTRIES: usize = 300;
pub const DB_FILE: &str = "rayclip.db";

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    /// text | link | email | color | image | file | files
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    /// Imagem guardada no banco (servida por `http://rayclip.localhost/img/<id>`).
    #[serde(default)]
    pub has_image: bool,
    #[serde(default)]
    pub image_width: Option<u32>,
    #[serde(default)]
    pub image_height: Option<u32>,
    #[serde(default)]
    pub source: Option<String>,
    /// PNG do ícone do app de origem (arquivo em `icons/`, não é conteúdo).
    #[serde(default)]
    pub source_icon: Option<String>,
    /// Texto reconhecido por OCR (apenas imagens).
    #[serde(default)]
    pub ocr_text: Option<String>,
    /// Miniatura (poster do GIF, capa de PDF/vídeo) guardada no banco.
    #[serde(default)]
    pub has_thumb: bool,
    /// A imagem é um GIF animado (precisa ser colado/salvo como GIF).
    #[serde(default)]
    pub gif: bool,
    /// Metadados do site quando a entrada é um link (título, favicon, capa).
    #[serde(default)]
    pub link: Option<crate::linkmeta::LinkMeta>,
    /// Formatos ricos originais (para colar como o Raycast: "every original format").
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub rtf: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
    pub created_at: i64,
    #[serde(default)]
    pub pinned: bool,
    /// Posição dentro do grupo "Fixados" (0 = topo). O usuário reordena com
    /// "Move Up/Down in Pinned", como no Raycast.
    #[serde(default)]
    pub pin_order: i64,
    /// Content fingerprint used to drop consecutive duplicates.
    #[serde(default)]
    pub hash: String,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entries (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL,
  title        TEXT NOT NULL,
  text         TEXT,
  paths        TEXT NOT NULL DEFAULT '[]',
  has_image    INTEGER NOT NULL DEFAULT 0,
  image_w      INTEGER,
  image_h      INTEGER,
  source       TEXT,
  source_icon  TEXT,
  ocr_text     TEXT,
  has_thumb    INTEGER NOT NULL DEFAULT 0,
  gif          INTEGER NOT NULL DEFAULT 0,
  link         TEXT,
  html         TEXT,
  rtf          TEXT,
  size_bytes   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  pinned       INTEGER NOT NULL DEFAULT 0,
  pin_order    INTEGER NOT NULL DEFAULT 0,
  hash         TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS entries_created ON entries(created_at DESC);
CREATE INDEX IF NOT EXISTS entries_hash ON entries(hash);
CREATE TABLE IF NOT EXISTS blobs (
  id    TEXT NOT NULL,
  kind  TEXT NOT NULL,
  bytes BLOB NOT NULL,
  PRIMARY KEY (id, kind)
);
CREATE TABLE IF NOT EXISTS meta (
  k TEXT PRIMARY KEY,
  v TEXT
);
";

const UPSERT: &str = "
INSERT INTO entries (id, kind, title, text, paths, has_image, image_w, image_h,
                     source, source_icon, ocr_text, has_thumb, gif, link, html, rtf,
                     size_bytes, created_at, pinned, pin_order, hash)
VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)
ON CONFLICT(id) DO UPDATE SET
  kind=excluded.kind, title=excluded.title, text=excluded.text,
  paths=excluded.paths, has_image=excluded.has_image, image_w=excluded.image_w,
  image_h=excluded.image_h, source=excluded.source,
  source_icon=excluded.source_icon, ocr_text=excluded.ocr_text,
  has_thumb=excluded.has_thumb, gif=excluded.gif, link=excluded.link, html=excluded.html,
  rtf=excluded.rtf, size_bytes=excluded.size_bytes,
  created_at=excluded.created_at, pinned=excluded.pinned,
  pin_order=excluded.pin_order, hash=excluded.hash";

#[derive(Default)]
pub struct Store {
    pub entries: Vec<Entry>,
    /// Clipboard sequence number we produced ourselves (do not re-capture).
    pub ignore_sequence: u32,
    pub dir: PathBuf,
    /// Limite configurável de entradas (vem das configurações).
    pub max_entries: usize,
    db: Option<Db>,
    /// `true` se o banco abriu de verdade (sem ele nada é persistido).
    pub encrypted: bool,
    /// `true` quando o banco não abriu ou a leitura falhou: neste estado o
    /// histórico em memória está **incompleto**, então `save()` nunca apaga
    /// linhas do banco — senão uma falha de leitura zeraria o histórico.
    pub degraded: bool,
    /// Motivo pelo qual a chave/banco não abriu (aparece na interface).
    pub key_error: Option<String>,
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        // Fail-closed: sem chave segura (CSPRNG + DPAPI) o banco não abre. O app
        // continua em memória e o motivo aparece em Configurações.
        let chave = crate::key::load_or_create(&dir);
        let mut entries_ok: Vec<Entry> = Vec::new();
        let db_path = dir.join(DB_FILE);
        let mut chave_erro: Option<String> = None;
        let db = match chave {
            Ok(key) => match Db::open(&db_path, &key) {
            Ok(db) => {
                if let Err(e) = db.exec(SCHEMA) {
                    crate::log(&format!("db: schema falhou: {e}"));
                }
                // Banco de versão anterior: a coluna de ordem dos fixados entra
                // depois. `ALTER TABLE` falha com "duplicate column" quando ela
                // já existe e isso é esperado (por isso o erro é ignorado).
                let _ = db.exec("ALTER TABLE entries ADD COLUMN pin_order INTEGER NOT NULL DEFAULT 0;");
                // `secure_delete` zera o conteúdo apagado (sem "lixo" recuperável).
                let _ = db.exec("PRAGMA secure_delete = ON;");
                let _ = db.exec("PRAGMA journal_mode = DELETE;");
                let _ = db.exec("PRAGMA synchronous = NORMAL;");
                Some(db)
            }
            Err(e) => {
                crate::log(&format!("db: nao abriu ({e}) - seguindo so em memoria"));
                None
            }
            },
            Err(motivo) => {
                crate::log(&format!("db: sem chave ({motivo}) - somente memoria"));
                chave_erro = Some(motivo);
                None
            }
        };
        // Sem banco ou com leitura falhando, marcamos "degradado": nada de
        // reconciliar (reconciliar apagaria tudo o que não veio na leitura).
        let degraded = match db.as_ref() {
            None => true,
            Some(d) => match read_all_checked(d) {
                Ok(v) => {
                    entries_ok = v;
                    false
                }
                Err(e) => {
                    crate::log(&format!("db: leitura falhou ({e}) - modo degradado"));
                    true
                }
            },
        };
        let mut entries = entries_ok;
        if entries.is_empty() && !degraded {
            if let Some(db) = db.as_ref() {
                let had_json = dir.join("store.json").exists();
                if had_json {
                    migrate_legacy(db, &dir);
                    entries = read_all(db);
                }
            }
        }
        crate::log(&format!(
            "db: {} entradas (cifrado={}) em {}",
            entries.len(),
            db.is_some(),
            db_path.display()
        ));
        Store {
            entries,
            ignore_sequence: 0,
            dir,
            max_entries: MAX_ENTRIES,
            encrypted: db.is_some(),
            degraded,
            db,
            key_error: chave_erro,
        }
    }

    /// Handle clonável do banco (usado pela importação em lote).
    pub fn db(&self) -> Option<Db> {
        self.db.clone()
    }

    /// Pasta de mídia derivada (favicon, capa de link, ícone de app). Não é
    /// conteúdo do clipboard — o conteúdo fica no banco.
    pub fn media_dir(&self) -> PathBuf {
        let d = self.dir.join("media");
        let _ = std::fs::create_dir_all(&d);
        d
    }

    pub fn set_max_entries(&mut self, n: usize) {
        self.max_entries = n.max(50);
        self.trim();
        self.save();
    }

    /// Remove entradas mais antigas que `days` (0 = não mexe). Preserva fixadas.
    pub fn purge_older_than(&mut self, days: u32) -> usize {
        if days == 0 {
            return 0;
        }
        let cutoff = now_ms() - (days as i64) * 86_400_000;
        let before = self.entries.len();
        let dropped: Vec<String> = self
            .entries
            .iter()
            .filter(|e| !e.pinned && e.created_at < cutoff)
            .map(|e| e.id.clone())
            .collect();
        self.entries.retain(|e| e.pinned || e.created_at >= cutoff);
        for id in &dropped {
            self.delete_row(id);
        }
        before - self.entries.len()
    }

    fn trim(&mut self) {
        while self.entries.len() > self.max_entries.max(50) {
            match self.entries.iter().rposition(|e| !e.pinned) {
                Some(idx) => {
                    let gone = self.entries.remove(idx);
                    self.delete_row(&gone.id);
                }
                None => break,
            }
        }
    }

    pub fn add(&mut self, entry: Entry) {
        let mut entry = entry;
        // Drop exact duplicates already present anywhere in history.
        if let Some(pos) = self.entries.iter().position(|e| e.hash == entry.hash) {
            let old = self.entries.remove(pos);
            // Mesmo conteúdo recopiado: **mantém o id** (bump para o topo sem
            // recriar a linha). Assim o pin, o OCR, os metadados do link e as
            // mídias continuam válidos e as URLs /img/<id> não mudam.
            entry.id = old.id;
            entry.pinned = old.pinned;
            // Recopiado continua na mesma posição dentro dos fixados.
            entry.pin_order = old.pin_order;
            // Recopiado: mantém o que já foi indexado (OCR, metadados do link).
            if entry.link.is_none() {
                entry.link = old.link;
            }
            if entry.ocr_text.is_none() {
                entry.ocr_text = old.ocr_text;
            }
            if entry.html.is_none() {
                entry.html = old.html;
            }
            if entry.rtf.is_none() {
                entry.rtf = old.rtf;
            }
            // Os blobs já estão gravados sob este id — não há nada a copiar.
            entry.has_image = entry.has_image || old.has_image;
            entry.has_thumb = entry.has_thumb || old.has_thumb;
            self.entries.insert(0, entry.clone());
            self.upsert(&entry);
            // Não apaga a linha antiga: é a mesma (mesmo id), já regravada acima.
        } else {
            self.entries.insert(0, entry.clone());
            self.upsert(&entry);
        }
        // Trim unpinned overflow.
        self.trim();
    }

    /// Fixa/desfixa uma entrada. Quem é fixado agora vai para o **topo** do
    /// grupo fixado (ordem 0), como no Raycast.
    pub fn toggle_pin(&mut self, id: &str) -> Option<bool> {
        let now_pinned = {
            let e = self.entries.iter_mut().find(|e| e.id == id)?;
            e.pinned = !e.pinned;
            e.pin_order = if e.pinned { i64::MIN } else { 0 };
            e.pinned
        };
        self.renumber_pins(if now_pinned { Some(id) } else { None });
        Some(now_pinned)
    }

    /// Sobe/desce uma entrada fixada dentro do grupo "Fixados".
    pub fn move_pinned(&mut self, id: &str, up: bool) -> bool {
        let mut order = self.pinned_ids();
        let Some(pos) = order.iter().position(|x| x == id) else {
            return false;
        };
        let target = if up {
            pos.checked_sub(1)
        } else if pos + 1 < order.len() {
            Some(pos + 1)
        } else {
            None
        };
        let Some(target) = target else { return false };
        order.swap(pos, target);
        self.write_pin_order(&order);
        true
    }

    /// Ids fixados na ordem exibida (pin_order; empate = mais recente antes).
    fn pinned_ids(&self) -> Vec<String> {
        let mut rows: Vec<(&Entry, i64)> = self
            .entries
            .iter()
            .filter(|e| e.pinned)
            .map(|e| (e, e.pin_order))
            .collect();
        rows.sort_by(|a, b| a.1.cmp(&b.1).then(b.0.created_at.cmp(&a.0.created_at)));
        rows.into_iter().map(|(e, _)| e.id.clone()).collect()
    }

    fn write_pin_order(&mut self, order: &[String]) {
        for (i, id) in order.iter().enumerate() {
            let want = i as i64;
            let snap = match self.entries.iter_mut().find(|e| &e.id == id) {
                Some(e) if e.pin_order != want => {
                    e.pin_order = want;
                    Some(e.clone())
                }
                _ => None,
            };
            if let Some(e) = snap {
                self.touch(&e);
            }
        }
    }

    /// Renumera os fixados de 0..n-1. `top` (quando existe) vira a posição 0.
    fn renumber_pins(&mut self, top: Option<&str>) {
        let mut order = self.pinned_ids();
        if let Some(top) = top {
            if let Some(pos) = order.iter().position(|x| x == top) {
                let t = order.remove(pos);
                order.insert(0, t);
            }
        }
        self.write_pin_order(&order);
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Grava a imagem (PNG) e a miniatura opcional de uma entrada.
    pub fn put_image(&mut self, id: &str, png: &[u8], thumb: Option<&[u8]>) {
        let Some(db) = self.db.as_ref() else { return };
        let _ = db.run(
            "INSERT INTO blobs (id, kind, bytes) VALUES (?1,'image',?2)
             ON CONFLICT(id,kind) DO UPDATE SET bytes=excluded.bytes",
            &[id.into(), png.into()],
        );
        if let Some(t) = thumb {
            let _ = db.run(
                "INSERT INTO blobs (id, kind, bytes) VALUES (?1,'thumb',?2)
                 ON CONFLICT(id,kind) DO UPDATE SET bytes=excluded.bytes",
                &[id.into(), t.into()],
            );
        }
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.has_image = true;
            if thumb.is_some() {
                e.has_thumb = true;
            }
            let e = e.clone();
            self.upsert(&e);
        }
    }

    pub fn put_thumb(&mut self, id: &str, thumb: &[u8]) {
        let Some(db) = self.db.as_ref() else { return };
        let _ = db.run(
            "INSERT INTO blobs (id, kind, bytes) VALUES (?1,'thumb',?2)
             ON CONFLICT(id,kind) DO UPDATE SET bytes=excluded.bytes",
            &[id.into(), thumb.into()],
        );
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.has_thumb = true;
            let e = e.clone();
            self.upsert(&e);
        }
    }

    pub fn image_bytes(&self, id: &str) -> Option<Vec<u8>> {
        self.blob(id, "image")
    }

    pub fn thumb_bytes(&self, id: &str) -> Option<Vec<u8>> {
        self.blob(id, "thumb")
    }

    fn blob(&self, id: &str, kind: &str) -> Option<Vec<u8>> {
        let db = self.db.as_ref()?;
        let row = db
            .query_opt(
                "SELECT bytes FROM blobs WHERE id = ?1 AND kind = ?2",
                &[id.into(), kind.into()],
            )
            .ok()??;
        row[0].blob()
    }

    /// Atualiza uma entrada já existente (OCR, link, fixar, etc.).
    pub fn touch(&self, entry: &Entry) {
        self.upsert(entry);
    }

    fn upsert(&self, e: &Entry) {
        let Some(db) = self.db.as_ref() else { return };
        let paths = serde_json::to_string(&e.paths).unwrap_or_else(|_| "[]".into());
        let link = e.link.as_ref().and_then(|l| serde_json::to_string(l).ok());
        let params = [
            Val::from(e.id.as_str()),
            Val::from(e.kind.as_str()),
            Val::from(e.title.as_str()),
            Val::from(e.text.clone()),
            Val::from(paths),
            Val::from(e.has_image),
            Val::from(e.image_width.map(i64::from)),
            Val::from(e.image_height.map(i64::from)),
            Val::from(e.source.clone()),
            Val::from(e.source_icon.clone()),
            Val::from(e.ocr_text.clone()),
            Val::from(e.has_thumb),
            Val::from(e.gif),
            Val::from(link),
            Val::from(e.html.clone()),
            Val::from(e.rtf.clone()),
            Val::from(e.size_bytes as i64),
            Val::from(e.created_at),
            Val::from(e.pinned),
            Val::from(e.pin_order),
            Val::from(e.hash.as_str()),
        ];
        if let Err(err) = db.run(UPSERT, &params) {
            crate::log(&format!("db: upsert {} falhou: {err}", e.id));
        }
    }

    fn delete_row(&self, id: &str) {
        let Some(db) = self.db.as_ref() else { return };
        let _ = db.run("DELETE FROM entries WHERE id = ?1", &[id.into()]);
        let _ = db.run("DELETE FROM blobs WHERE id = ?1", &[id.into()]);
    }

    /// Remove uma entrada (e seus blobs).
    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        self.delete_row(id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        if let Some(db) = self.db.as_ref() {
            let _ = db.run("DELETE FROM entries", &[]);
            let _ = db.run("DELETE FROM blobs", &[]);
            let _ = db.exec("VACUUM;");
        }
    }

    /// Reconcilia o banco com o que está em memória (após mudanças em massa).
    pub fn save(&self) {
        let Some(db) = self.db.as_ref() else { return };
        if self.degraded {
            // Histórico em memória pode estar incompleto: apagar o que "sobrou"
            // destruiria dados. Só grava as linhas conhecidas, sem remover nada.
            let _ = db.exec("BEGIN IMMEDIATE;");
            for e in &self.entries {
                self.upsert(e);
            }
            let _ = db.exec("COMMIT;");
            crate::log("db: degradado - save() sem remocao");
            return;
        }
        let ids: Vec<String> = self.entries.iter().map(|e| e.id.clone()).collect();
        let keep: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        let existing: Vec<String> = db
            .query("SELECT id FROM entries", &[])
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| r[0].text())
            .filter(|id| !keep.contains(id.as_str()))
            .collect();
        let _ = db.exec("BEGIN IMMEDIATE;");
        for id in &existing {
            self.delete_row(id);
        }
        for e in &self.entries {
            self.upsert(e);
        }
        let _ = db.exec("COMMIT;");
    }
}

fn read_all(db: &Db) -> Vec<Entry> {
    read_all_checked(db).unwrap_or_default()
}

fn read_all_checked(db: &Db) -> Result<Vec<Entry>, String> {
    let rows = db.query(
        "SELECT id, kind, title, text, paths, has_image, image_w, image_h, source,
                source_icon, ocr_text, has_thumb, gif, link, html, rtf, size_bytes,
                created_at, pinned, pin_order, hash
         FROM entries ORDER BY created_at DESC",
        &[],
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Entry {
            id: r[0].text().unwrap_or_default(),
            kind: r[1].text().unwrap_or_else(|| "text".into()),
            title: r[2].text().unwrap_or_default(),
            text: r[3].text(),
            paths: r[4]
                .text()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            has_image: r[5].int() != 0,
            image_width: r[6].text().and_then(|v| v.parse().ok()),
            image_height: r[7].text().and_then(|v| v.parse().ok()),
            source: r[8].text(),
            source_icon: r[9].text(),
            ocr_text: r[10].text(),
            has_thumb: r[11].int() != 0,
            gif: r[12].int() != 0,
            link: r[13].text().and_then(|s| serde_json::from_str(&s).ok()),
            html: r[14].text(),
            rtf: r[15].text(),
            size_bytes: r[16].int().max(0) as u64,
            created_at: r[17].int(),
            pinned: r[18].int() != 0,
            pin_order: r[19].int(),
            hash: r[20].text().unwrap_or_default(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Migração do formato antigo (store.json + PNGs soltos em images/)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyEntry {
    id: String,
    kind: String,
    title: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    image_path: Option<String>,
    #[serde(default)]
    image_width: Option<u32>,
    #[serde(default)]
    image_height: Option<u32>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    source_icon: Option<String>,
    #[serde(default)]
    ocr_text: Option<String>,
    #[serde(default)]
    thumb: Option<String>,
    #[serde(default)]
    link: Option<crate::linkmeta::LinkMeta>,
    #[serde(default)]
    html: Option<String>,
    #[serde(default)]
    rtf: Option<String>,
    #[serde(default)]
    size_bytes: u64,
    created_at: i64,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    hash: String,
}

fn migrate_legacy(db: &Db, dir: &Path) {
    let json_path = dir.join("store.json");
    let Ok(raw) = std::fs::read_to_string(&json_path) else {
        return;
    };
    let Ok(old): Result<Vec<LegacyEntry>, _> = serde_json::from_str(&raw) else {
        crate::log("db: store.json antigo ilegivel, nao migrado");
        return;
    };
    let total = old.len();
    let mut with_image = 0usize;
    let _ = db.exec("BEGIN IMMEDIATE;");
    for o in old {
        let image = o
            .image_path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .filter(|b| !b.is_empty());
        let thumb = o
            .thumb
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .filter(|b| !b.is_empty());
        // Ícone de app e favicon/capa de link não são conteúdo: vão para media/.
        let source_icon = o.source_icon.as_ref().and_then(|p| move_media(dir, p));
        let link = o.link.map(|mut l| {
            l.image_path = l.image_path.and_then(|p| move_media(dir, &p));
            l
        });
        let entry = Entry {
            id: o.id,
            kind: o.kind,
            title: o.title,
            text: o.text,
            paths: o.paths,
            has_image: image.is_some(),
            image_width: o.image_width,
            image_height: o.image_height,
            source: o.source,
            source_icon,
            ocr_text: o.ocr_text,
            has_thumb: thumb.is_some(),
            gif: image.as_deref().is_some_and(|b| b.starts_with(b"GIF8")),
            link,
            html: o.html,
            rtf: o.rtf,
            size_bytes: o.size_bytes,
            created_at: o.created_at,
            pinned: o.pinned,
            pin_order: 0,
            hash: o.hash,
        };
        let paths = serde_json::to_string(&entry.paths).unwrap_or_else(|_| "[]".into());
        let link_json = entry
            .link
            .as_ref()
            .and_then(|l| serde_json::to_string(l).ok());
        let params = [
            Val::from(entry.id.as_str()),
            Val::from(entry.kind.as_str()),
            Val::from(entry.title.as_str()),
            Val::from(entry.text.clone()),
            Val::from(paths),
            Val::from(entry.has_image),
            Val::from(entry.image_width.map(i64::from)),
            Val::from(entry.image_height.map(i64::from)),
            Val::from(entry.source.clone()),
            Val::from(entry.source_icon.clone()),
            Val::from(entry.ocr_text.clone()),
            Val::from(entry.has_thumb),
            Val::from(entry.gif),
            Val::from(link_json),
            Val::from(entry.html.clone()),
            Val::from(entry.rtf.clone()),
            Val::from(entry.size_bytes as i64),
            Val::from(entry.created_at),
            Val::from(entry.pinned),
            Val::from(entry.hash.as_str()),
        ];
        if db.run(UPSERT, &params).is_err() {
            continue;
        }
        if let Some(bytes) = image.as_ref() {
            with_image += 1;
            let _ = db.run(
                "INSERT OR REPLACE INTO blobs (id, kind, bytes) VALUES (?1,'image',?2)",
                &[entry.id.as_str().into(), bytes.into()],
            );
            if let Some(p) = &o.image_path {
                let _ = std::fs::remove_file(p);
            }
        }
        if let Some(bytes) = thumb.as_ref() {
            let _ = db.run(
                "INSERT OR REPLACE INTO blobs (id, kind, bytes) VALUES (?1,'thumb',?2)",
                &[entry.id.as_str().into(), bytes.into()],
            );
            if let Some(p) = &o.thumb {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    let _ = db.exec("COMMIT;");
    // Nada de conteúdo em claro sobrando: o JSON (com todo o texto antigo) e
    // os PNGs soltos são apagados — já estão (cifrados) no banco.
    let _ = std::fs::remove_file(&json_path);
    let images = dir.join("images");
    if let Ok(read) = std::fs::read_dir(&images) {
        let mut left = 0;
        for e in read.flatten() {
            if std::fs::remove_file(e.path()).is_ok() {
                left += 1;
            }
        }
        let _ = std::fs::remove_dir(&images);
        if left > 0 {
            crate::log(&format!("db: {left} arquivos antigos removidos de images/"));
        }
    }
    crate::log(&format!(
        "db: migradas {total} entradas do formato antigo ({with_image} imagens)"
    ));
}

/// Move um arquivo de mídia derivada para `media/` e devolve o novo caminho.
fn move_media(dir: &Path, path: &str) -> Option<String> {
    let src = Path::new(path);
    if !src.is_file() {
        return Some(path.to_string());
    }
    let media = dir.join("media");
    let _ = std::fs::create_dir_all(&media);
    let name = src.file_name()?.to_string_lossy().to_string();
    let dst = media.join(name);
    if std::fs::rename(src, &dst).is_ok() || dst.is_file() {
        Some(dst.to_string_lossy().to_string())
    } else {
        Some(path.to_string())
    }
}

pub fn app_dir(base: &Path) -> PathBuf {
    let dir = base.join("clipboard");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, created_at: i64) -> Entry {
        Entry {
            id: id.to_string(),
            kind: "text".into(),
            title: id.to_string(),
            text: Some(id.to_string()),
            paths: vec![],
            has_image: false,
            image_width: None,
            image_height: None,
            source: None,
            source_icon: None,
            ocr_text: None,
            has_thumb: false,
            gif: false,
            link: None,
            html: None,
            rtf: None,
            size_bytes: 1,
            created_at,
            pinned: false,
            pin_order: 0,
            hash: id.to_string(),
        }
    }

    #[test]
    fn fixados_entram_no_topo_e_reordenam() {
        // Sem banco: `touch` não grava nada e a lógica de ordem é testável.
        let mut s = Store::default();
        s.entries = vec![entry("a", 3), entry("b", 2), entry("c", 1)];

        s.toggle_pin("a");
        s.toggle_pin("c");
        // O último fixado vai para o topo do grupo.
        assert_eq!(s.pinned_ids(), vec!["c".to_string(), "a".to_string()]);

        assert!(s.move_pinned("a", true));
        assert_eq!(s.pinned_ids(), vec!["a".to_string(), "c".to_string()]);
        // Já está no topo: não mexe.
        assert!(!s.move_pinned("a", true));
        assert!(s.move_pinned("a", false));
        assert_eq!(s.pinned_ids(), vec!["c".to_string(), "a".to_string()]);
        // Entrada não fixada não entra na ordem.
        assert!(!s.move_pinned("b", true));

        // Desfixar tira do grupo.
        s.toggle_pin("c");
        assert_eq!(s.pinned_ids(), vec!["a".to_string()]);
        assert_eq!(s.entries.iter().find(|e| e.id == "c").unwrap().pin_order, 0);
    }
}
