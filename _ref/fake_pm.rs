//! Simulador de gerenciador de senhas para testar a captura do Rayclip.
//!
//! Cria uma janela oculta (para ser o **dono** do clipboard, como um app real),
//! escreve um texto e, opcionalmente, os marcadores que os apps usam para dizer
//! "não registre isto". Compilado com `rustc` e copiado com nomes de
//! gerenciadores (Bitwarden.exe, 1Password.exe…) para exercitar a lista
//! `IGNORED_APPS` do capture.rs.
//!
//! Uso: fake_pm.exe <modo> <texto>
//!   text        texto puro
//!   exclude     texto + ExcludeClipboardContentFromMonitorProcessing
//!   viewer      texto + Clipboard Viewer Ignore
//!   caninclude0 texto + CanIncludeInClipboardHistory = 0
//!   caninclude1 texto + CanIncludeInClipboardHistory = 1
//!   files       lista de arquivos (CF_HDROP falsificado não: usa texto + formato FileNameW)

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;

type Hwnd = isize;
type Hbitmap = isize;

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(cls: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: isize,
        inst: isize,
        param: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, msg: u32, w: usize, l: isize) -> isize;
    fn OpenClipboard(h: Hwnd) -> i32;
    fn EmptyClipboard() -> i32;
    fn CloseClipboard() -> i32;
    fn SetClipboardData(fmt: u32, data: isize) -> isize;
    fn RegisterClipboardFormatW(name: *const u16) -> u32;
    fn GetModuleHandleW(name: *const u16) -> isize;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(h: isize) -> *mut u8;
    fn GlobalUnlock(h: isize) -> i32;
}

#[repr(C)]
struct WndClassW {
    style: u32,
    wndproc: usize,
    cls_extra: i32,
    wnd_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu_name: *const u16,
    class_name: *const u16,
}

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

extern "system" fn wndproc(h: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    unsafe { DefWindowProcW(h, msg, w, l) }
}

fn wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = std::ffi::OsStr::new(s).encode_wide().collect();
    v.push(0);
    v
}

unsafe fn janela() -> Hwnd {
    let class = wide("RayclipFakePasswordManager");
    let cls = WndClassW {
        style: 0,
        wndproc: wndproc as usize,
        cls_extra: 0,
        wnd_extra: 0,
        instance: GetModuleHandleW(std::ptr::null()),
        icon: 0,
        cursor: 0,
        background: 0,
        menu_name: std::ptr::null(),
        class_name: class.as_ptr(),
    };
    RegisterClassW(&cls);
    CreateWindowExW(
        0,
        class.as_ptr(),
        class.as_ptr(),
        0, // sem estilo: janela que nunca aparece
        0,
        0,
        0,
        0,
        0,
        0,
        GetModuleHandleW(std::ptr::null()),
        std::ptr::null_mut(),
    )
}

/// Copia bytes para memoria global (como o clipboard exige) e devolve o handle.
unsafe fn global(bytes: &[u8]) -> isize {
    let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
    if h == 0 {
        return 0;
    }
    let p = GlobalLock(h);
    if p.is_null() {
        return 0;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
    GlobalUnlock(h);
    h
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let modo = args.get(1).cloned().unwrap_or_else(|| "text".into());
    let texto = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "senha-simulada-123".into());

    unsafe {
        // `nowin*`: copia sem janela nenhuma — o Windows não sabe o dono
        // (GetClipboardOwner == NULL), então só os marcadores protegem.
        let sem_janela = modo.starts_with("nowin");
        let hwnd = if sem_janela { 0 } else { janela() };
        if OpenClipboard(hwnd) == 0 {
            eprintln!("nao abriu o clipboard");
            std::process::exit(2);
        }
        EmptyClipboard();
        let mut utf16: Vec<u8> = Vec::new();
        for u in texto.encode_utf16() {
            utf16.extend_from_slice(&u.to_le_bytes());
        }
        utf16.extend_from_slice(&[0, 0]);
        SetClipboardData(CF_UNICODETEXT, global(&utf16));

        let marca = |nome: &str| -> u32 {
            let w = wide(nome);
            RegisterClipboardFormatW(w.as_ptr())
        };
        match modo.trim_start_matches("nowin_").trim_start_matches("nowin") {
            // Apps reais publicam um DWORD 1 (ou dados vazios) nesses formatos:
            // `SetClipboardData(fmt, NULL)` sozinho e "rendering atrasado" e
            // costuma falhar, entao usamos um payload minimo de verdade.
            "exclude" => {
                let r = SetClipboardData(
                    marca("ExcludeClipboardContentFromMonitorProcessing"),
                    global(&1u32.to_le_bytes()),
                );
                if r == 0 {
                    eprintln!("SetClipboardData(exclude) falhou");
                }
            }
            "viewer" => {
                let r = SetClipboardData(marca("Clipboard Viewer Ignore"), global(&1u32.to_le_bytes()));
                if r == 0 {
                    eprintln!("SetClipboardData(viewer) falhou");
                }
            }
            "caninclude0" | "caninclude1" => {
                let v: u32 = if modo == "caninclude0" { 0 } else { 1 };
                SetClipboardData(marca("CanIncludeInClipboardHistory"), global(&v.to_le_bytes()));
            }
            _ => {}
        }
        CloseClipboard();
        // O dono do clipboard e a janela deste processo; se ele sair na hora o
        // Windows perde o dono — espera um instante para o Rayclip consultar.
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let _ = hwnd;
    }
    println!("{modo}: {texto}");
}
