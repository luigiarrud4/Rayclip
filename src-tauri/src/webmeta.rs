//! Metadados que **só um navegador de verdade** consegue ler.
//!
//! Alguns sites ficam atrás do desafio do Cloudflare: `https://klipy.com/gifs/…`
//! responde **403 com a página "Just a moment…"** para qualquer cliente que não
//! seja um Chromium real (medido com `curl` + UA e cabeçalhos de navegação: 403
//! sempre). O HTML desses sites nem tem `og:image` — a página é montada no
//! cliente, então baixar os bytes e procurar `<meta>` não resolve.
//!
//! A saída é usar o WebView2 que o app já tem: uma janela **invisível** carrega
//! a página, um script injetado lê as tags `og:image`/`og:video` (ou o maior
//! `<img>`/`<video>` do DOM) e devolve as URLs por uma rota do protocolo
//! próprio (`rayclip://webmeta/<b64>`), que é o canal de volta sem depender de
//! IPC injetado em página de terceiro.
//!
//! **Só roda sob demanda**: uma por vez, chamada quando o usuário abre uma
//! entrada de link que ficou sem capa. Nada acontece em segundo plano.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Última resposta recebida pela rota `rayclip://webmeta/<b64>`.
static RESPOSTA: Mutex<Option<String>> = Mutex::new(None);
/// Uma janela escondida por vez: duas ao mesmo tempo brigam por memória e foco.
static OCUPADO: AtomicBool = AtomicBool::new(false);

const ESPERA_MAX: u32 = 60; // 60 * 150 ms = 9 s
const PASSO_MS: u64 = 150;

/// Já existe uma janela de metadados aberta?
pub fn ocupado() -> bool {
    OCUPADO.load(Ordering::SeqCst)
}

/// Chamada pela rota do protocolo: a página devolveu as URLs encontradas.
pub fn receber(b64: &str) {
    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(b64) else {
        return;
    };
    let Ok(json) = String::from_utf8(bytes) else {
        return;
    };
    *RESPOSTA.lock().unwrap_or_else(|e| e.into_inner()) = Some(json);
}

/// Script injetado a cada passo. Idempotente: marca `__rayclipMeta` antes de
/// navegar, senão cada `eval` repetiria a navegação.
///
/// A volta é por `location.replace` para a rota do protocolo próprio — a página
/// é de outro domínio, então `fetch` para o app seria barrado pelo CORS e o IPC
/// do Tauri não é injetado em origem remota.
const SCRIPT: &str = r#"
(function () {
  try {
    if (window.__rayclipMeta) return;
    function meta(p) {
      var e = document.querySelector('meta[property="' + p + '"]') ||
              document.querySelector('meta[name="' + p + '"]');
      return e && e.content ? e.content : null;
    }
    var out = [];
    var v = meta('og:video:secure_url') || meta('og:video:url') || meta('og:video');
    if (v) out.push(v);
    var i = meta('og:image:secure_url') || meta('og:image') || meta('twitter:image');
    if (i) out.push(i);
    if (!out.length) {
      // Página montada no cliente: o maior <img> animado do DOM.
      var melhor = null, area = 0;
      var imgs = document.querySelectorAll('img');
      for (var k = 0; k < imgs.length; k++) {
        var s = (imgs[k].currentSrc || imgs[k].src || '').toLowerCase();
        if (!/\.(gif|webp)(\?|$)/.test(s)) continue;
        var a = (imgs[k].naturalWidth || 0) * (imgs[k].naturalHeight || 0);
        if (a > area) { area = a; melhor = s; }
      }
      if (melhor) out.push(melhor);
      var vids = document.querySelectorAll('video');
      for (var j = 0; j < vids.length; j++) {
        var sv = (vids[j].currentSrc || vids[j].src || '').toLowerCase();
        if (/\.(mp4|webm)(\?|$)/.test(sv) || /data:video/.test(sv)) { out.push(sv); break; }
      }
    }
    if (!out.length) return;
    window.__rayclipMeta = 1;
    var b = btoa(unescape(encodeURIComponent(JSON.stringify(out.slice(0, 4)))))
      .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    location.replace('http://rayclip.localhost/webmeta/' + b);
  } catch (e) {}
})()
"#;

/// Abre a janela invisível, espera as URLs e fecha. Bloqueia a thread chamadora
/// (chame de dentro de uma thread, nunca da UI).
pub fn buscar(app: &AppHandle, url: &str) -> Option<Vec<String>> {
    let alvo: tauri::Url = url.parse().ok()?;
    if OCUPADO.swap(true, Ordering::SeqCst) {
        return None;
    }
    *RESPOSTA.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let rotulo = format!(
        "webmeta-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    // A janela **tem** de nascer na thread principal: criar WebView2 fora dela
    // devolve `HRESULT(0x8007139F)` (ERROR_INVALID_STATE) e nenhuma janela
    // aparece. O resto (esperar o desafio da Cloudflare) fica na thread de
    // trabalho, senão a interface congelaria por segundos.
    let (envio, recebe) = std::sync::mpsc::channel::<Result<WebviewWindow, String>>();
    let app_janela = app.clone();
    let rot_janela = rotulo.clone();
    let pedido = app.run_on_main_thread(move || {
        let mut construtor =
            WebviewWindowBuilder::new(&app_janela, &rot_janela, WebviewUrl::External(alvo))
                .visible(false)
                .skip_taskbar(true)
                .decorations(false)
                .inner_size(1100.0, 800.0);
        // Mesmos argumentos da janela principal: o ambiente do WebView2 é um só
        // por processo, e argumentos diferentes fazem o `build()` falhar com
        // `HRESULT(0x8007139F)` (tauri#11144).
        if let Some(extra) = crate::args_webview2() {
            construtor = construtor.additional_browser_args(&extra);
        }
        let r = construtor.build().map_err(|e| e.to_string());
        let _ = envio.send(r);
    });
    let janela = if pedido.is_err() {
        crate::log("webmeta: thread principal recusou o pedido");
        None
    } else {
        match recebe.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(j)) => Some(j),
            Ok(Err(e)) => {
                crate::log(&format!("webmeta: janela falhou: {e}"));
                None
            }
            Err(_) => {
                crate::log("webmeta: janela nao respondeu em 10 s");
                None
            }
        }
    };

    let mut achado: Option<String> = None;
    let mut n_passos = 0u32;
    if let Some(j) = &janela {
        for _ in 0..ESPERA_MAX {
            std::thread::sleep(Duration::from_millis(PASSO_MS));
            n_passos += 1;
            let _ = j.eval(SCRIPT);
            if let Some(json) = RESPOSTA.lock().unwrap_or_else(|e| e.into_inner()).take() {
                achado = Some(json);
                break;
            }
        }
    }
    if janela.is_some() {
        let app_fim = app.clone();
        let rot_fim = rotulo.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = app_fim.get_webview_window(&rot_fim) {
                let _ = w.destroy();
            }
        });
    }
    if achado.is_none() {
        crate::log(&format!(
            "webmeta: sem resposta depois de {n_passos} passos (titulo {:?})",
            janela.as_ref().map(|j| j.title().unwrap_or_default()).unwrap_or_default()
        ));
    }
    OCUPADO.store(false, Ordering::SeqCst);
    achado.and_then(|j| serde_json::from_str::<Vec<String>>(&j).ok())
}
