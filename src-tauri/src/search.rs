//! Busca e ordenação **no Rust** (§8.44).
//!
//! Antes a página recebia todas as entradas (1,8 MB, ~33 ms de serialização) e
//! filtrava em JS. Aqui o filtro e a ordem acontecem antes de responder, então
//! o IPC leva só o que vai aparecer.
//!
//! As regras são as mesmas que estavam em `src/lib/format.ts`:
//!
//! 1. texto/vínculo antes de imagem que só casou pelo OCR;
//! 2. dentro do tipo, fixados primeiro;
//! 3. casamento exato, depois início, depois começo de palavra, depois meio;
//! 4. recência (a lista de origem já vem da mais recente).
//!
//! O teste `ordem_igual_a_antiga` guarda a paridade com a versão em JS.

use crate::store::Entry;

/// Minúsculas e sem acento: "Café São Paulo" casa com "cafe sao".
///
/// O JS usa `normalize("NFD")`; aqui a tabela cobre Latin-1 + Latin Extended-A,
/// que é o que aparece em texto de clipboard (português, espanhol, francês,
/// alemão, escandinavo, polonês, tcheco…).
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let t = match c {
            'À'..='Å' | 'à'..='å' | 'Ā' | 'ā' | 'Ă' | 'ă' | 'Ą' | 'ą' => 'a',
            'Ç' | 'ç' | 'Ć' | 'ć' | 'Ĉ' | 'ĉ' | 'Ċ' | 'ċ' | 'Č' | 'č' => 'c',
            'Ď' | 'ď' | 'Đ' | 'đ' => 'd',
            'È'..='Ë' | 'è'..='ë' | 'Ē' | 'ē' | 'Ĕ' | 'ĕ' | 'Ė' | 'ė' | 'Ę' | 'ę' | 'Ě' | 'ě' => 'e',
            'Ĝ' | 'ĝ' | 'Ğ' | 'ğ' | 'Ġ' | 'ġ' | 'Ģ' | 'ģ' => 'g',
            'Ĥ' | 'ĥ' | 'Ħ' | 'ħ' => 'h',
            'Ì'..='Ï' | 'ì'..='ï' | 'Ĩ' | 'ĩ' | 'Ī' | 'ī' | 'Ĭ' | 'ĭ' | 'Į' | 'į' | 'İ' | 'ı' => 'i',
            'Ĵ' | 'ĵ' => 'j',
            'Ķ' | 'ķ' => 'k',
            'Ĺ' | 'ĺ' | 'Ļ' | 'ļ' | 'Ľ' | 'ľ' | 'Ŀ' | 'ŀ' | 'Ł' | 'ł' => 'l',
            'Ñ' | 'ñ' | 'Ń' | 'ń' | 'Ņ' | 'ņ' | 'Ň' | 'ň' => 'n',
            'Ò'..='Ö' | 'ò'..='ö' | 'Ō' | 'ō' | 'Ŏ' | 'ŏ' | 'Ő' | 'ő' => 'o',
            'Ŕ' | 'ŕ' | 'Ŗ' | 'ŗ' | 'Ř' | 'ř' => 'r',
            'Ś' | 'ś' | 'Ŝ' | 'ŝ' | 'Ş' | 'ş' | 'Š' | 'š' => 's',
            'Ţ' | 'ţ' | 'Ť' | 'ť' | 'Ŧ' | 'ŧ' => 't',
            'Ù'..='Ü' | 'ù'..='ü' | 'Ũ' | 'ũ' | 'Ū' | 'ū' | 'Ŭ' | 'ŭ' | 'Ů' | 'ů' | 'Ű' | 'ű' | 'Ų' | 'ų' => 'u',
            'Ŵ' | 'ŵ' => 'w',
            'Ý' | 'ý' | 'ÿ' | 'Ŷ' | 'ŷ' | 'Ÿ' => 'y',
            'Ź' | 'ź' | 'Ż' | 'ż' | 'Ž' | 'ž' => 'z',
            'ß' => 's',
            _ => c.to_ascii_lowercase(),
        };
        out.push(t);
    }
    out
}

/// Filtro das "chips" de tipo da barra superior.
pub fn matches_filter(e: &Entry, filter: &str) -> bool {
    match filter {
        "text" => e.kind == "text",
        "image" => e.kind == "image",
        "video" => e.kind == "video",
        "file" => matches!(
            e.kind.as_str(),
            "file" | "files" | "pdf" | "video" | "audio" | "archive"
        ),
        "link" => e.kind == "link",
        "email" => e.kind == "email",
        "color" => e.kind == "color",
        _ => true,
    }
}

/// Quanto de cada campo entra na busca. É o mesmo corte da projeção que ia por
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
    crate::win32::thread_below_normal();
    for e in entries {
        let _ = campos(e);
    }
}

fn casa(c: &Campos, q: &str) -> bool {
    c.visiveis.iter().any(|f| f.contains(q)) || c.ocr.as_deref().is_some_and(|o| o.contains(q))
}

#[cfg(test)]
pub fn matches_query(e: &Entry, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    casa(&campos(e), q)
}

/// True quando a busca só casou com o texto lido de dentro da imagem.
#[cfg(test)]
pub fn matched_only_in_ocr(e: &Entry, q: &str) -> bool {
    let c = campos(e);
    if q.is_empty() {
        return false;
    }
    let Some(ocr) = c.ocr.as_deref() else { return false };
    !c.visiveis.iter().any(|f| f.contains(q)) && ocr.contains(q)
}

/// Posição do casamento dentro de um campo:
/// 0 = igual, 1 = começa com, 2 = começa uma palavra, 3 = no meio, 9 = não achou.
fn hit_rank(hay: &str, q: &str) -> u8 {
    if hay.is_empty() {
        return 9;
    }
    let Some(i) = hay.find(q) else { return 9 };
    if i == 0 {
        return if hay.len() == q.len() { 0 } else { 1 };
    }
    // `char_indices` para não cortar no meio de um acento.
    let antes = hay[..i].chars().next_back().unwrap_or(' ');
    if " \t\n-_/\\.,;:()[]{}'\"<>@#$%&*+=!?|~^`".contains(antes) {
        2
    } else {
        3
    }
}

fn score(e: &Entry, c: &Campos, q: &str) -> u32 {
    let visivel = c.visiveis.iter().map(|f| hit_rank(f, q)).min().unwrap_or(9);
    // Só casou no texto lido de dentro da imagem → fila de trás.
    let tier = if visivel == 9 { 1 } else { 0 };
    let rank = if tier == 0 {
        visivel
    } else {
        hit_rank(c.ocr.as_deref().unwrap_or(""), q)
    };
    tier * 1000 + if e.pinned { 0 } else { 100 } + rank as u32 * 10
}

/// Aplica filtro + busca + ordem e recorta a página pedida.
///
/// Devolve `(itens da página, total de casados)`. A ordem é **estável**, então
/// empate mantém a recência que veio do banco.
pub fn filtrar(
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
}

#[cfg(test)]
mod testes {
    use super::*;

    fn e(id: &str, kind: &str, title: &str, text: &str, ts: i64) -> Entry {
        Entry {
            id: id.to_string(),
            kind: kind.to_string(),
            title: title.to_string(),
            text: Some(text.to_string()),
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
            size_bytes: 0,
            created_at: ts,
            pinned: false,
            pin_order: 0,
            hash: String::new(),
        }
    }

    #[test]
    fn fold_tira_acento_e_caixa() {
        assert_eq!(fold("Café São Paulo ÀÉÎÕÜ Ç"), "cafe sao paulo aeiou c");
    }

    #[test]
    fn busca_casa_com_acento_nos_dois_sentidos() {
        let x = e("1", "text", "", "coração de mãe", 1);
        assert!(matches_query(&x, &fold("coracao")));
        let y = e("2", "text", "", "coracao de mae", 1);
        assert!(matches_query(&y, &fold("coração")));
    }

    #[test]
    fn filtro_por_tipo() {
        let img = e("1", "image", "", "", 1);
        let link = e("2", "link", "https://x", "", 1);
        let pdf = e("3", "pdf", "a.pdf", "", 1);
        assert!(!matches_filter(&img, "text"));
        assert!(matches_filter(&img, "image"));
        assert!(matches_filter(&link, "link"));
        let video = e("4", "video", "a.mp4", "", 1);
        assert!(matches_filter(&video, "video"));
        assert!(!matches_filter(&video, "image"));
        assert!(matches_filter(&pdf, "file"));
        assert!(matches_filter(&pdf, "all"));
    }

    #[test]
    fn ordem_igual_a_antiga() {
        // Mesmo conjunto do teste em JS: exato > prefixo > começo de palavra >
        // meio, e texto antes de OCR.
        let exato = e("exato", "text", "nota", "nota", 10);
        let prefixo = e("prefixo", "text", "nota fiscal", "", 20);
        let palavra = e("palavra", "text", "minha nota fiscal", "", 30);
        let meio = e("meio", "text", "annotar", "", 40);
        let mut ocr = e("ocr", "image", "", "", 50);
        ocr.ocr_text = Some("nota".to_string());
        let todos = vec![meio, palavra, ocr, prefixo, exato]; // ordem por recência
        let (itens, total) = filtrar(todos, "nota", "all", 0, 50);
        assert_eq!(total, 5);
        let ids: Vec<&str> = itens.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["exato", "prefixo", "palavra", "meio", "ocr"]);
    }

    #[test]
    fn fixado_sobe_dentro_do_mesmo_tipo() {
        let mut a = e("a", "text", "nota a", "", 99);
        a.pinned = true;
        let b = e("b", "text", "nota b", "", 100);
        let (itens, _) = filtrar(vec![b, a], "nota", "all", 0, 10);
        assert_eq!(itens[0].id, "a");
    }

    #[test]
    fn paginacao_devolve_total_e_recorta() {
        let todos: Vec<Entry> = (0..10)
            .map(|i| e(&format!("e{i}"), "text", "abc", "", 100 - i as i64))
            .collect();
        let (p0, total) = filtrar(todos.clone(), "abc", "all", 0, 4);
        assert_eq!((p0.len(), total), (4, 10));
        let (p2, _) = filtrar(todos, "abc", "all", 8, 4);
        assert_eq!(p2.len(), 2);
    }

    #[test]
    fn so_ocr_marca_a_entrada() {
        let mut img = e("i", "image", "foto", "", 1);
        img.ocr_text = Some("recibo do mercado".to_string());
        assert!(matched_only_in_ocr(&img, "mercado"));
        // "foto" aparece no título, então não é "só OCR".
        assert!(!matched_only_in_ocr(&img, "foto"));
    }
}
