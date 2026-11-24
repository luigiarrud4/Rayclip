"""Cache dos campos dobrados: a primeira consulta custava 198 ms dobrando 1.683
entradas; com o cache, cada consulta é só varredura de string."""

import io

P = "C:/Users/Lugia/Projects/rayclip/src-tauri/src/search.rs"
s = io.open(P, encoding="utf-8").read()

old = '''/// Campos visíveis (os mesmos do `matchesQuery` em JS). O título sintético das
/// imagens ("Image (data)") **não** entra: ele muda com o idioma e procurar
/// "image" casando com toda imagem era ruído.
fn campos_visiveis(e: &Entry) -> [String; 4] {
    [
        fold(&e.title),
        fold(e.text.as_deref().unwrap_or("")),
        fold(&e.paths.join(" ")),
        fold(e.source.as_deref().unwrap_or("")),
    ]
}

pub fn matches_query(e: &Entry, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    campos_visiveis(e).iter().any(|f| f.contains(q))
        || fold(e.ocr_text.as_deref().unwrap_or("")).contains(q)
}

/// True quando a busca só casou com o texto lido de dentro da imagem.
pub fn matched_only_in_ocr(e: &Entry, q: &str) -> bool {
    if q.is_empty() || e.ocr_text.as_deref().unwrap_or("").is_empty() {
        return false;
    }
    let so_ocr = !campos_visiveis(e).iter().any(|f| f.contains(q));
    so_ocr && fold(e.ocr_text.as_deref().unwrap_or("")).contains(q)
}'''

new = '''/// Quanto de cada campo entra na busca. É o mesmo corte da projeção que ia por
/// IPC (4 KB): procurar além disso nunca funcionou, nem antes nem agora.
const BUSCA_MAX: usize = 4 * 1024;

/// Campos já dobrados (minúsculos, sem acento) de uma entrada.
///
/// Dobrar 1.683 entradas custava **198 ms por consulta** — inaceitável a cada
/// tecla. Aqui o resultado fica em cache: a varredura passa a ser só
/// `contains` sobre ~1 KB por campo.
struct Campos {
    /// `created_at` + `hash` do conteúdo: muda quando a entrada é editada ou
    /// renomeada, e aí o cache é refeito.
    stamp: String,
    visiveis: [String; 4],
    ocr: Option<String>,
}

fn dobra_campo(txt: &str) -> String {
    if txt.len() <= BUSCA_MAX {
        fold(txt)
    } else {
        fold(crate::cut_utf8_pub(txt, BUSCA_MAX))
    }
}

fn monta_campos(e: &Entry) -> Campos {
    Campos {
        stamp: format!("{}|{}", e.created_at, e.hash),
        visiveis: [
            // O título sintético das imagens ("Image (data)") **não** entra: ele
            // muda com o idioma e procurar "image" casando com toda imagem era
            // ruído.
            dobra_campo(&e.title),
            dobra_campo(e.text.as_deref().unwrap_or("")),
            dobra_campo(&e.paths.join(" ")),
            dobra_campo(e.source.as_deref().unwrap_or("")),
        ],
        ocr: e.ocr_text.as_deref().map(dobra_campo),
    }
}

fn cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Campos>>>
{
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Campos>>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Campos dobrados da entrada, do cache quando possível.
fn campos(e: &Entry) -> std::sync::Arc<Campos> {
    let stamp = format!("{}|{}", e.created_at, e.hash);
    if let Ok(mapa) = cache().lock() {
        if let Some(c) = mapa.get(&e.id) {
            if c.stamp == stamp {
                return c.clone();
            }
        }
    }
    let novo = std::sync::Arc::new(monta_campos(e));
    if let Ok(mut mapa) = cache().lock() {
        // Limite de segurança: histórico gigante não pode inflar a memória.
        if mapa.len() > 6000 {
            mapa.clear();
        }
        mapa.insert(e.id.clone(), novo.clone());
    }
    novo
}

/// Aquece o cache (chamado numa thread de fundo ao abrir o histórico): a
/// primeira tecla digitada não paga a dobra de tudo.
pub fn aquecer(entries: &[Entry]) {
    crate::thread_below_normal();
    for e in entries {
        let _ = campos(e);
    }
}

fn casa(c: &Campos, q: &str) -> bool {
    c.visiveis.iter().any(|f| f.contains(q)) || c.ocr.as_deref().is_some_and(|o| o.contains(q))
}

pub fn matches_query(e: &Entry, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    casa(&campos(e), q)
}

/// True quando a busca só casou com o texto lido de dentro da imagem.
pub fn matched_only_in_ocr(e: &Entry, q: &str) -> bool {
    let c = campos(e);
    if q.is_empty() {
        return false;
    }
    let Some(ocr) = c.ocr.as_deref() else { return false };
    !c.visiveis.iter().any(|f| f.contains(q)) && ocr.contains(q)
}'''
assert old in s
s = s.replace(old, new, 1)

# score passa a usar os campos do cache
old = '''fn score(e: &Entry, q: &str) -> u32 {
    let visivel = campos_visiveis(e)
        .iter()
        .map(|f| hit_rank(f, q))
        .min()
        .unwrap_or(9);
    // Só casou no texto lido de dentro da imagem → fila de trás.
    let tier = if visivel == 9 { 1 } else { 0 };
    let rank = if tier == 0 {
        visivel
    } else {
        hit_rank(&fold(e.ocr_text.as_deref().unwrap_or("")), q)
    };
    tier * 1000 + if e.pinned { 0 } else { 100 } + rank as u32 * 10
}'''
new = '''fn score(e: &Entry, c: &Campos, q: &str) -> u32 {
    let visivel = c.visiveis.iter().map(|f| hit_rank(f, q)).min().unwrap_or(9);
    // Só casou no texto lido de dentro da imagem → fila de trás.
    let tier = if visivel == 9 { 1 } else { 0 };
    let rank = if tier == 0 {
        visivel
    } else {
        hit_rank(c.ocr.as_deref().unwrap_or(""), q)
    };
    tier * 1000 + if e.pinned { 0 } else { 100 } + rank as u32 * 10
}'''
assert old in s
s = s.replace(old, new, 1)

# filtrar: casa/score pelo cache e só ordena os casados
old = '''pub fn filtrar(
    mut todos: Vec<Entry>,
    query: &str,
    filter: &str,
    offset: usize,
    limit: usize,
) -> (Vec<Entry>, usize) {
    let q = fold(query.trim());
    if !q.is_empty() {
        // Ordena antes de recortar: a página 0 precisa ser o topo do ranking,
        // não o topo da lista por data.
        todos.sort_by_key(|e| score(e, &q));
    }
    let casados: Vec<Entry> = todos
        .into_iter()
        .filter(|e| matches_filter(e, filter) && matches_query(e, &q))
        .collect();
    let total = casados.len();
    let itens = casados.into_iter().skip(offset).take(limit).collect();
    (itens, total)
}'''
new = '''pub fn filtrar(
    todos: Vec<Entry>,
    query: &str,
    filter: &str,
    offset: usize,
    limit: usize,
) -> (Vec<Entry>, usize) {
    let q = fold(query.trim());
    let mut casados: Vec<(u32, Entry)> = Vec::new();
    for e in todos {
        if !matches_filter(&e, filter) {
            continue;
        }
        let c = campos(&e);
        if !q.is_empty() && !casa(&c, &q) {
            continue;
        }
        // Sem busca a nota é 0 e a ordem vem do banco (fixados, recência).
        let nota = if q.is_empty() { 0 } else { score(&e, &c, &q) };
        casados.push((nota, e));
    }
    if !q.is_empty() {
        // `sort_by_key` é estável: empate mantém a recência que veio do banco.
        casados.sort_by_key(|(nota, _)| *nota);
    }
    let total = casados.len();
    let itens = casados
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, e)| e)
        .collect();
    (itens, total)
}'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("search.rs: cache")
