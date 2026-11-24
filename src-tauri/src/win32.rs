//! Win32 helpers: clipboard owner detection, foreground window tracking, paste keystroke.

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::{
    CloseHandle, HWND, INVALID_HANDLE_VALUE, MAX_PATH, POINT, RECT,
};
use windows_sys::Win32::System::DataExchange::{GetClipboardOwner, GetClipboardSequenceNumber};
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, CreateMutexW, GetCurrentProcessId, GetCurrentThread, GetCurrentThreadId,
    OpenProcess, QueryFullProcessImageNameW, SetThreadPriority, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, THREAD_PRIORITY_BELOW_NORMAL,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsWindow,
    SetForegroundWindow, ShowWindow, SwitchToThisWindow, SW_HIDE, SW_SHOW,
};

/// Monotonic clipboard change counter. Cheap: no clipboard read required.
pub fn sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Name of the process that currently owns the clipboard (e.g. "vivaldi").
#[allow(dead_code)]
pub fn clipboard_owner_name() -> Option<String> {
    unsafe {
        let hwnd = GetClipboardOwner();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        process_name(pid)
    }
}

#[allow(dead_code)]
pub fn process_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; MAX_PATH as usize];
        let mut len = MAX_PATH;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let stem = std::path::Path::new(&full)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())?;
        Some(stem)
    }
}

/// Caminho completo do executável de um processo.
pub fn process_image_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// PID do processo pai (via Toolhelp32).
pub fn process_parent(pid: u32) -> Option<u32> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry = std::mem::zeroed::<PROCESSENTRY32W>();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut parent = None;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == pid {
                    parent = Some(entry.th32ParentProcessID);
                    break;
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        parent
    }
}

/// O processo é o nosso, ou um filho nosso (o WebView2 usa processos filhos,
/// ex.: msedgewebview2.exe). Nesses casos a origem é o próprio Rayclip.
pub fn is_self_or_child(pid: u32) -> bool {
    if pid == current_pid() {
        return true;
    }
    let mut p = process_parent(pid);
    let mut guard = 0;
    while let Some(parent) = p {
        if parent == current_pid() {
            return true;
        }
        p = process_parent(parent);
        guard += 1;
        if guard > 8 {
            break;
        }
    }
    false
}

/// Nome amigável para exibir como "Source".
pub fn friendly_source_name(exe_path: &str) -> String {
    let stem = std::path::Path::new(exe_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".into());
    match stem.as_str() {
        "explorer" => "File Explorer".into(),
        "WindowsTerminal" => "Terminal".into(),
        "powershell" | "pwsh" => "PowerShell".into(),
        "WINWORD" => "Word".into(),
        "EXCEL" => "Excel".into(),
        "chrome" => "Chrome".into(),
        "firefox" => "Firefox".into(),
        "msedge" => "Edge".into(),
        "Code" => "VS Code".into(),
        "notepad" => "Notepad".into(),
        _ => stem,
    }
}

/// Processo da janela em primeiro plano: (PID, exe). Fallback para quando o
/// dono do clipboard já morreu (apps que copiam e fecham, ex.: `Set-Clipboard`).
pub fn foreground_full() -> Option<(u32, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let path = process_image_path(pid)?;
        Some((pid, path))
    }
}

/// Dono do clipboard: (PID, caminho do executável).
pub fn clipboard_owner_full() -> Option<(u32, String)> {
    unsafe {
        let hwnd = GetClipboardOwner();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let path = process_image_path(pid)?;
        Some((pid, path))
    }
}

/// Remove o blur acrílico (volta ao fundo sólido).
fn set_acrylic_none(hwnd: isize) {
    unsafe {
        let mut policy = AccentPolicy {
            state: 0, // ACCENT_DISABLED
            flags: 0,
            gradient_color: 0,
            animation_id: 0,
        };
        let mut data = WinCompAttrData {
            attrib: WCA_ACCENT_POLICY,
            data: &mut policy as *mut _ as *mut core::ffi::c_void,
            size: std::mem::size_of::<AccentPolicy>(),
        };
        if let Some(f) = set_wca_proc() {
            let _ = f(hwnd as *mut core::ffi::c_void, &mut data);
        }
    }
}

/// Backdrop translúcido do Windows 11. `kind`: 1 = desligado, 2 = Mica,
/// 3 = Acrylic, 4 = Mica aplicativo.
#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WinCompAttrData {
    attrib: usize,
    data: *mut core::ffi::c_void,
    size: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut core::ffi::c_void;
    fn GetProcAddress(module: *mut core::ffi::c_void, name: *const u8) -> *mut core::ffi::c_void;
}

type SetWcaFn = unsafe extern "system" fn(*mut core::ffi::c_void, *mut WinCompAttrData) -> i32;

/// `SetWindowCompositionAttribute` não é exportada de forma pública: vem de
/// `GetProcAddress` em user32.dll.
fn set_wca_proc() -> Option<SetWcaFn> {
    static PROC: std::sync::OnceLock<Option<SetWcaFn>> = std::sync::OnceLock::new();
    *PROC.get_or_init(|| unsafe {
        let user32: Vec<u16> = "user32.dll\0".encode_utf16().collect();
        let module = GetModuleHandleW(user32.as_ptr());
        if module.is_null() {
            return None;
        }
        let name = b"SetWindowCompositionAttribute\0";
        let f = GetProcAddress(module, name.as_ptr());
        if f.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut core::ffi::c_void, SetWcaFn>(f))
        }
    })
}

const WCA_ACCENT_POLICY: usize = 19;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

/// Blur "acrílico" via API não documentada: funciona mesmo com os "efeitos de
/// transparência" do Windows desligados, ao contrário do Mica do DWM.
/// `tint` em 0xAABBGGRR.
fn set_acrylic(hwnd: isize, tint: u32) {
    unsafe {
        let mut policy = AccentPolicy {
            state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
            flags: 2, // ACCENT_FLAG_DRAW_ALL_BORDERS
            gradient_color: tint,
            animation_id: 0,
        };
        let mut data = WinCompAttrData {
            attrib: WCA_ACCENT_POLICY,
            data: &mut policy as *mut _ as *mut core::ffi::c_void,
            size: std::mem::size_of::<AccentPolicy>(),
        };
        if let Some(f) = set_wca_proc() {
            let _ = f(hwnd as *mut core::ffi::c_void, &mut data);
        }
    }
}

pub fn apply_backdrop(hwnd: isize, kind: u32) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmExtendFrameIntoClientArea(
            hwnd: *mut core::ffi::c_void,
            margins: *const core::ffi::c_void,
        ) -> i32;
    }
    #[repr(C)]
    struct Margins {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    }
    unsafe {
        // Estende o vidro por toda a janela (sem isso o Mica só aparece na
        // moldura). -1 em todas as margens = cliente inteiro; 0 desfaz.
        let m = if kind > 1 {
            Margins {
                left: -1,
                right: -1,
                top: -1,
                bottom: -1,
            }
        } else {
            Margins {
                left: 0,
                right: 0,
                top: 0,
                bottom: 0,
            }
        };
        let _ = DwmExtendFrameIntoClientArea(
            hwnd as *mut core::ffi::c_void,
            &m as *const _ as *const core::ffi::c_void,
        );
        let mut value = kind as i32;
        let _ = DwmSetWindowAttribute(
            hwnd as *mut core::ffi::c_void,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &mut value as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
        if kind > 1 {
            // tint escuro (0xAABBGGRR): A=0x55 (33%), cor #0a0a0c
            set_acrylic(hwnd, 0x55_0C_0A_0A);
        } else {
            set_acrylic_none(hwnd);
        }
        let mut dark = 1i32;
        let _ = DwmSetWindowAttribute(
            hwnd as *mut core::ffi::c_void,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &mut dark as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

pub fn current_pid() -> u32 {
    unsafe { GetCurrentProcessId() }
}

fn class_name(hwnd: HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

fn title_of(hwnd: HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 256];
    let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

/// Abre uma URL/arquivo com o programa padrão **sem** passar por `cmd /C start`.
/// `cmd` interpretava `&`, `|`, `^` e `%VAR%` do conteúdo da área de transferência
/// como comando (uma entrada "link" com `&calc.exe` executaria o calc).
pub fn shell_open(target: &str) -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let wide = |s: &str| -> Vec<u16> {
        let mut v: Vec<u16> = s.encode_utf16().collect();
        v.push(0);
        v
    };
    let verb = wide("open");
    let file = wide(target);
    unsafe {
        let r = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        );
        r as isize > 32
    }
}

/// Esconde a janela **agora**, sem passar pelo event loop do Tauri.
/// `WebviewWindow::hide()` despacha para a thread principal; quando ela está
/// ocupada (pintando a janela que acabou de abrir) o hide atrasa centenas de
/// ms. O `ShowWindow` direto é instantâneo.
pub fn hide_now(h: isize) {
    if h != 0 {
        unsafe { ShowWindow(h as HWND, SW_HIDE) };
    }
}

/// Janela **raiz** de um handle (sobe pelos filhos do WebView2 até a janela
/// de verdade). É assim que se sabe se um clique caiu na nossa janela: o ponto
/// pode estar numa janela filha que nem pertence ao nosso processo.
pub fn root_of(h: isize) -> isize {
    if h == 0 {
        return 0;
    }
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOT};
    unsafe {
        let root = GetAncestor(h as HWND, GA_ROOT);
        if root.is_null() {
            h
        } else {
            root as isize
        }
    }
}

/// Classe de uma janela por handle.
pub fn class_of_hwnd(h: isize) -> String {
    if h == 0 {
        return String::new();
    }
    class_name(h as HWND)
}

/// PID dono de uma janela (0 se desconhecida).
pub fn pid_of_hwnd(h: isize) -> u32 {
    pid_of(h as HWND)
}

fn pid_of(hwnd: HWND) -> u32 {
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// O primeiro plano é nosso — ou é uma janela popup do próprio WebView2
/// (dropdown do `<select>`, DevTools), que roda em outro processo mas pertence
/// à interface.
#[allow(dead_code)]
pub fn foreground_is_self() -> bool {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, GW_OWNER};
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        if pid_of(fg) == GetCurrentProcessId() {
            return true;
        }
        // Popups do WebView2 (dropdown de <select>, DevTools) não têm título e
        // rodam em outro processo; janelas do Chrome/VS Code têm título e não
        // devem contar como "nós".
        if class_name(fg).starts_with("Chrome_WidgetWin") && title_of(fg).is_empty() {
            return true;
        }
        let owner = GetWindow(fg, GW_OWNER);
        !owner.is_null() && pid_of(owner) == GetCurrentProcessId()
    }
}

/// Diz se o primeiro plano é de **outro** aplicativo. Devolve false quando:
/// - não há janela em primeiro plano (transição, não dá para decidir);
/// - a janela é nossa (inclui o diálogo "Salvar como" e o WebView2);
/// - a janela é popup de WebView2 sem título;
/// - a janela pertence a um processo filho/owned pelo nosso.
///
/// Só um "true" confirmado (duas leituras seguidas) deve esconder a janela.
pub fn foreground_is_foreign() -> bool {
    hwnd_is_foreign(unsafe { GetForegroundWindow() as isize })
}

/// Mesma regra, para uma janela qualquer (o hook já entrega o hwnd novo).
pub fn hwnd_is_foreign(h: isize) -> bool {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, IsWindowVisible, GW_OWNER};
        let fg = h as HWND;
        if fg.is_null() {
            return false; // desconhecido: não esconde
        }
        if IsWindowVisible(fg) == 0 {
            return false; // janela fantasma (ex.: overlay que já fechou)
        }
        if pid_of(fg) == GetCurrentProcessId() {
            return false;
        }
        if class_name(fg).starts_with("Chrome_WidgetWin") && title_of(fg).is_empty() {
            return false;
        }
        let owner = GetWindow(fg, GW_OWNER);
        if !owner.is_null() && pid_of(owner) == GetCurrentProcessId() {
            return false;
        }
        true
    }
}

/// O cursor do mouse está sobre a nossa janela? Enquanto o ponteiro estiver em
/// cima dela nunca escondemos: um console que pisca (reg.exe/taskkill), um menu
/// ou um clique dentro do app não podem fazer a janela desaparecer.
pub fn cursor_over_window(hwnd: isize) -> bool {
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut p) == 0 {
            return false;
        }
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd as HWND, &mut r) == 0 {
            return false;
        }
        p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
    }
}

/// Descrição curta da janela em primeiro plano (para o log de depuração).
pub fn foreground_desc() -> String {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return "fg=(none)".into();
        }
        format!(
            "fg=hwnd={} pid={} class={} title={:?}",
            fg as isize,
            pid_of(fg),
            class_name(fg),
            title_of(fg)
        )
    }
}

/// Remembers which window had focus, so "Paste" can target it after we hide.
pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

#[allow(dead_code)]
pub fn window_alive(hwnd: isize) -> bool {
    unsafe { IsWindow(hwnd as HWND) != 0 }
}

/// Force a window to the foreground even from a background process.
/// Uses the AttachThreadInput trick because Windows blocks SetForegroundWindow
/// for processes that did not receive the last input event.
pub fn force_foreground(hwnd: isize) {
    unsafe {
        let h = hwnd as HWND;
        if h.is_null() || IsWindow(h) == 0 {
            return;
        }
        ShowWindow(h, SW_SHOW);
        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        let fg_thread = if fg.is_null() {
            0
        } else {
            GetWindowThreadProcessId(fg, &mut fg_pid)
        };
        let cur = GetCurrentThreadId();
        if fg_thread != 0 && fg_thread != cur {
            AttachThreadInput(cur, fg_thread, 1);
            SwitchToThisWindow(h, 1);
            SetForegroundWindow(h);
            SetFocus(h);
            AttachThreadInput(cur, fg_thread, 0);
        } else {
            SwitchToThisWindow(h, 1);
            SetForegroundWindow(h);
            SetFocus(h);
        }
    }
}

/// Dá ao nosso processo o direito de trazer qualquer janela para a frente.
///
/// Sem isso o `SetForegroundWindow` da colagem é recusado quando o Windows
/// aplica a "foreground lock" e sobra só o truque de AttachThreadInput (mais
/// lento). O Raycast faz o mesmo (`AllowSetForegroundWindow` no helper).
pub fn allow_foreground_any() {
    use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    const ASFW_ANY: u32 = 0xFFFF_FFFF;
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
}

/// Um único `SendInput` com os quatro eventos do Ctrl+V.
///
/// `keybd_event` fazia quatro chamadas separadas: outro teclado podia se
/// intrometer no meio (e o alvo recebia "Ctrl" solto). O `SendInput` entra como
/// um bloco atômico — mais rápido e sem corrida com o usuário digitando.
fn send_ctrl_v() -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };
    let tecla = |vk: u16, flags: u32| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let entradas = [
        tecla(VK_CONTROL, 0),
        tecla(VK_V, 0),
        tecla(VK_V, KEYEVENTF_KEYUP),
        tecla(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(
            entradas.len() as u32,
            entradas.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    }
}

/// Foca a janela alvo e cola com Ctrl+V. Devolve `true` se a colagem foi enviada
/// com o alvo em primeiro plano.
///
/// Caminho rápido: quando o alvo **já** está em primeiro plano (o caso normal —
/// escondemos a janela e o Windows devolve o foco na hora) não há espera
/// nenhuma, só o `SendInput`. Antes eram 8 ms fixos + até 280 ms de sondagem.
pub fn paste_into(hwnd: isize) -> bool {
    let t0 = std::time::Instant::now();
    let h = hwnd as HWND;
    let mut esperou = 0u32;
    unsafe {
        if h.is_null() || IsWindow(h) == 0 {
            // Sem alvo conhecido: cola onde o foco estiver.
            send_ctrl_v();
            crate::log("paste_into: sem alvo, colado no foco atual");
            return true;
        }
        if GetForegroundWindow() != h {
            // Uma tentativa só, com o truque de AttachThreadInput, e depois
            // sondagem curta de 1 ms (o Windows devolve o foco em 1–3 ms).
            force_foreground(hwnd);
            while GetForegroundWindow() != h && esperou < 30 {
                std::thread::sleep(std::time::Duration::from_millis(1));
                esperou += 1;
            }
        }
        let ok = GetForegroundWindow() == h;
        if ok {
            send_ctrl_v();
        }
        crate::log(&format!(
            "paste_into: alvo={} fg_ok={} espera={}ms total={:?}",
            hwnd,
            ok,
            esperou,
            t0.elapsed()
        ));
        ok
    }
}

/// Prioridade abaixo do normal para a thread atual. O backfill de OCR roda em
/// segundo plano: indexar imagem não pode competir com a janela que o usuário
/// acabou de abrir (o Windows dá a vez a quem está na frente).
pub fn thread_below_normal() {
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

const ERROR_ALREADY_EXISTS: u32 = 183;
/// Single-instance guard: only one process may own the clipboard history store.
pub fn already_running() -> bool {
    unsafe {
        let name: Vec<u16> = r"Local\RayclipSingletonV1"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if h.is_null() {
            return false;
        }
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

/// Nome do executável que é dono do clipboard (para listas de exclusão).
pub fn clipboard_owner_process() -> Option<String> {
    use windows_sys::Win32::System::DataExchange::GetClipboardOwner;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let hwnd = GetClipboardOwner();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd as _, &mut pid);
        if pid == 0 {
            return None;
        }
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if proc.is_null() {
            return None;
        }
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        let _ = CloseHandle(proc);
        if ok == 0 {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        Some(
            full.rsplit(char::from(92))
                .next()
                .unwrap_or(&full)
                .to_string(),
        )
    }
}

/// Diálogo "Salvar como" do Windows (IFileSaveDialog). Devolve o caminho
/// escolhido ou None se o usuário cancelar.
pub fn save_dialog(default_name: &str, ext: &str, title: &str) -> Option<String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        Common::COMDLG_FILTERSPEC, FileSaveDialog, IFileSaveDialog, FOS_FORCEFILESYSTEM,
        FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let filter = match ext {
        "jpg" | "jpeg" => ("JPEG image".to_string(), "*.jpg;*.jpeg".to_string()),
        "webp" => ("WebP image".into(), "*.webp".into()),
        "bmp" => ("Bitmap image".into(), "*.bmp".into()),
        "gif" => ("GIF image".into(), "*.gif".into()),
        "png" => ("PNG image".into(), "*.png".into()),
        "txt" => ("Text file".into(), "*.txt".into()),
        "md" => ("Markdown".into(), "*.md".into()),
        "json" => ("JSON file".into(), "*.json".into()),
        "csv" => ("CSV file".into(), "*.csv".into()),
        "html" | "htm" => ("HTML file".into(), "*.html;*.htm".into()),
        "rtf" => ("Rich Text document".into(), "*.rtf".into()),
        "url" => ("Internet shortcut".into(), "*.url".into()),
        other => (
            format!("{} file", other.to_uppercase()),
            format!("*.{other}"),
        ),
    };
    let (label, spec) = filter;
    let w_label = wide(&label);
    let w_spec = wide(&spec);

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let owns_com = hr.is_ok();

        let result = (|| -> Option<String> {
            let dialog: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            let _ = dialog.SetTitle(&HSTRING::from(title));
            let filters = [COMDLG_FILTERSPEC {
                pszName: PCWSTR(w_label.as_ptr()),
                pszSpec: PCWSTR(w_spec.as_ptr()),
            }];
            let _ = dialog.SetFileTypes(&filters);
            let _ = dialog.SetDefaultExtension(&HSTRING::from(ext));
            let _ = dialog.SetFileName(&HSTRING::from(default_name));
            // `SetOptions` SUBSTITUI as opcoes padrao: passar so FORCEFILESYSTEM
            // derrubava OVERWRITEPROMPT e o dialogo apagava arquivos existentes
            // sem perguntar. Le as atuais e acrescenta o que falta.
            let mut opts = dialog.GetOptions().unwrap_or_default();
            opts.0 |= FOS_FORCEFILESYSTEM.0 | FOS_OVERWRITEPROMPT.0 | FOS_PATHMUSTEXIST.0;
            let _ = dialog.SetOptions(opts);
            if dialog.Show(None).is_err() {
                return None; // cancelado
            }
            let item = dialog.GetResult().ok()?;
            let raw = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let path = raw.to_string().ok()?;
            CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
            Some(path)
        })();

        if owns_com {
            CoUninitialize();
        }
        result
    }
}

/// Diálogo para escolher **uma pasta** (IFileOpenDialog + FOS_PICKFOLDERS).
pub fn pick_folder(title: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let owns_com = hr.is_ok();
        let result = (|| -> Option<String> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            let _ = dialog.SetTitle(&HSTRING::from(title));
            let _ = dialog.SetOptions(windows::Win32::UI::Shell::FILEOPENDIALOGOPTIONS(
                FOS_PICKFOLDERS.0 | FOS_FORCEFILESYSTEM.0,
            ));
            if dialog.Show(None).is_err() {
                return None; // cancelado
            }
            let item = dialog.GetResult().ok()?;
            let raw = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let path = raw.to_string().ok()?;
            CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
            Some(path)
        })();
        if owns_com {
            CoUninitialize();
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Segunda instância: avisa a que já está rodando para abrir a janela
// ---------------------------------------------------------------------------

const ACTIVATE_EVENT: &str = r"Local\RayclipActivateV1";

/// Chamado pela instância que acabou de ser executada: pede à instância viva
/// para mostrar a janela (atalho clicado duas vezes, menu Iniciar, etc.).
pub fn signal_activate() {
    use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};
    unsafe {
        let name: Vec<u16> = ACTIVATE_EVENT
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h = CreateEventW(std::ptr::null(), 1, 0, name.as_ptr());
        crate::log(&format!(
            "activate: sinalizando (handle ok={})",
            !h.is_null()
        ));
        if !h.is_null() {
            SetEvent(h);
        }
    }
}

/// Roda `f` toda vez que outra instância pedir para abrir a janela.
pub fn start_activate_watch<F: Fn() + Send + 'static>(f: F) {
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
    std::thread::spawn(move || unsafe {
        let name: Vec<u16> = ACTIVATE_EVENT
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // Manual-reset, criado aqui primeiro (a instância viva é a dona).
        let h = CreateEventW(std::ptr::null(), 1, 0, name.as_ptr());
        crate::log(&format!("activate-watch: criado ok={}", !h.is_null()));
        if h.is_null() {
            return;
        }
        loop {
            let hit = WaitForSingleObject(h, 500);
            if hit == WAIT_OBJECT_0 {
                crate::log("activate: sinal recebido");
                use windows_sys::Win32::System::Threading::ResetEvent;
                ResetEvent(h);
                f();
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Hook de primeiro plano (EVENT_SYSTEM_FOREGROUND)
// ---------------------------------------------------------------------------

static FG_CALLBACK: std::sync::OnceLock<Box<dyn Fn(isize) + Send + Sync>> =
    std::sync::OnceLock::new();

static CLICK_CB: std::sync::OnceLock<Box<dyn Fn(isize) + Send + Sync>> = std::sync::OnceLock::new();

unsafe extern "system" fn click_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, WindowFromPoint, MSLLHOOKSTRUCT,
    };
    if code >= 0 {
        let msg = wparam as u32;
        // Botão esquerdo, direito, do meio ou extra pressionado.
        if msg == 0x0201 || msg == 0x0204 || msg == 0x0207 || msg == 0x020B {
            let info = &*(lparam as *const MSLLHOOKSTRUCT);
            let target = WindowFromPoint(info.pt);
            if let Some(cb) = CLICK_CB.get() {
                cb(target as isize);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// Instala um hook **global de mouse** (`WH_MOUSE_LL`) que avisa em que janela
/// o botão foi pressionado. É o caminho mais rápido para esconder: o app sai
/// no instante do clique, mesmo quando o destino demora a ganhar o foco.
/// O callback roda na thread do hook e precisa ser barato.
pub fn on_mouse_down<F: Fn(isize) + Send + Sync + 'static>(f: F) {
    let _ = CLICK_CB.set(Box::new(f));
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, MSG, WH_MOUSE_LL,
        };
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(click_proc), std::ptr::null_mut(), 0);
        crate::log(&format!("mouse-hook: instalado ok={}", !hook.is_null()));
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn fg_proc(
    _hook: windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    idobject: i32,
    idchild: i32,
    _thread: u32,
    _time: u32,
) {
    // Só troca de janela em primeiro plano na raiz (não subobjetos/filhos).
    if event != 0x0003 /* EVENT_SYSTEM_FOREGROUND */ || idobject != 0 || idchild != 0 {
        return;
    }
    if let Some(cb) = FG_CALLBACK.get() {
        cb(hwnd as isize);
    } else {
        crate::log("hook: callback sem destino");
    }
}

/// Instala um hook **do sistema** (`SetWinEventHook`) que avisa na hora em que
/// outra janela vira primeiro plano — sem polling. O callback roda numa thread
/// própria com message loop; ele deve apenas sinalizar (nada de mexer na
/// janela daqui).
pub fn on_foreground_change<F: Fn(isize) + Send + Sync + 'static>(f: F) {
    let _ = FG_CALLBACK.set(Box::new(f));
    std::thread::spawn(|| unsafe {
        use windows_sys::Win32::UI::Accessibility::SetWinEventHook;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, TranslateMessage, MSG,
        };
        let hook = SetWinEventHook(
            0x0003, // EVENT_SYSTEM_FOREGROUND
            0x0003,
            std::ptr::null_mut(),
            Some(fg_proc),
            0,
            0,
            0x0000, // WINEVENT_OUTOFCONTEXT
        );
        crate::log(&format!("hook: instalado ok={}", !hook.is_null()));
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

/// Janelas que aparecem e somem em milissegundos (console de `reg.exe`,
/// `taskkill`, `cmd`) e que por isso **nunca** podem esconder o app.
pub fn foreground_is_transient() -> bool {
    hwnd_is_transient(unsafe { GetForegroundWindow() as isize })
}

/// Janela em primeiro plano agora (0 = nenhuma).
pub fn foreground_hwnd() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

/// A janela ainda existe e está visível?
pub fn is_window_visible(h: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    h != 0 && unsafe { IsWindowVisible(h as HWND) } != 0
}

/// Console que aparece e some em milissegundos (reg.exe, taskkill, cmd).
pub fn hwnd_is_transient(h: isize) -> bool {
    let class = class_name(h as HWND);
    matches!(
        class.as_str(),
        "ConsoleWindowClass" | "CASCADIA_HOSTING_WINDOW_CLASS" | "PseudoConsoleWindow"
    )
}

static CLIP_CB: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

/// `WM_CLIPBOARDUPDATE`: o Windows avisa **no instante** em que o clipboard
/// muda (mesma técnica do listener do Raycast). O polling de sequência só
/// descobre a mudança até 320 ms depois — e, se duas cópias acontecem nesse
/// intervalo, a primeira se perde de vez, porque a sequência já é a da segunda.
/// O callback roda na thread da janela de mensagens e deve apenas sinalizar.
pub fn on_clipboard_change<F: Fn() + Send + Sync + 'static>(f: F) {
    let _ = CLIP_CB.set(Box::new(f));
    std::thread::spawn(|| unsafe {
        use windows_sys::Win32::System::DataExchange::AddClipboardFormatListener;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage, MSG,
            WNDCLASSW,
        };
        const HWND_MESSAGE: isize = -3;
        const WM_CLIPBOARDUPDATE: u32 = 0x031D;

        let cls: Vec<u16> = "RayclipClipQueue\0".encode_utf16().collect();
        let inst = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(clip_proc);
        wc.hInstance = inst;
        wc.lpszClassName = cls.as_ptr();
        RegisterClassW(&wc);

        let title: Vec<u16> = "rayclip clip\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            0,
            cls.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE as HWND,
            std::ptr::null_mut(),
            inst,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            crate::log("clip-listener: janela nao criada (fica o polling)");
            return;
        }
        let ok = AddClipboardFormatListener(hwnd);
        crate::log(&format!("clip-listener: registrado ok={}", ok != 0));
        if ok == 0 {
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_CLIPBOARDUPDATE {
                if let Some(cb) = CLIP_CB.get() {
                    cb();
                }
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

#[link(name = "user32")]
extern "system" {
    fn DefWindowProcW(
        hwnd: *mut core::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
}

unsafe extern "system" fn clip_proc(
    hwnd: *mut core::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
