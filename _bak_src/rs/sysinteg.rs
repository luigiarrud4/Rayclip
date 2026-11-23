//! Integrações com o Windows que exigem registro/processos:
//! `DisabledHotkeys` (para liberar o Win+V), inicialização automática e
//! reinício do Explorer.

use std::process::Command;

const ADVANCED: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "Rayclip";

fn reg(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("reg");
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // Sem isto o Windows abre uma janela de console (cmd) que pisca e **rouba o
    // primeiro plano** — era o que fazia o app perder o foco ao abrir Configurações.
    let out = hidden(&mut cmd).output().map_err(|e| format!("reg: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            "reg falhou".into()
        } else {
            err
        })
    }
}

/// Letras hoje desativadas em `DisabledHotkeys`.
fn disabled_hotkeys() -> String {
    let out = reg(&["query", ADVANCED, "/v", "DisabledHotkeys"]).unwrap_or_default();
    for line in out.lines() {
        if line.contains("DisabledHotkeys") {
            if let Some(idx) = line.find("REG_SZ") {
                return line[idx + "REG_SZ".len()..].trim().to_string();
            }
        }
    }
    String::new()
}

/// Liga/desliga o Win+V do Windows (mesma técnica do CopyQ/Ditto: a lista de
/// letras que o Explorer não deve capturar em `Win+<letra>`).
pub fn set_replace_win_v(disable: bool) -> Result<(), String> {
    let mut letters: Vec<char> = disabled_hotkeys()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    letters.retain(|c| *c != 'V');
    if disable {
        letters.push('V');
    }
    letters.sort_unstable();
    letters.dedup();

    if letters.is_empty() {
        let _ = reg(&["delete", ADVANCED, "/v", "DisabledHotkeys", "/f"]);
        return Ok(());
    }
    let value: String = letters.into_iter().collect();
    reg(&[
        "add",
        ADVANCED,
        "/v",
        "DisabledHotkeys",
        "/t",
        "REG_SZ",
        "/d",
        &value,
        "/f",
    ])?;
    Ok(())
}

pub fn replace_win_v_active() -> bool {
    disabled_hotkeys().to_uppercase().contains('V')
}

/// Reinicia o Explorer para aplicar o `DisabledHotkeys` (fecha janelas abertas).
pub fn restart_explorer() -> Result<(), String> {
    let mut kill = Command::new("taskkill");
    kill.args(["/f", "/im", "explorer.exe"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let _ = hidden(&mut kill).output();
    std::thread::sleep(std::time::Duration::from_millis(900));
    Command::new("explorer.exe")
        .spawn()
        .map_err(|e| format!("explorer: {e}"))?;
    Ok(())
}

/// "Efeitos de transparência" do Windows estão ligados? Sem isso o Mica e o
/// acrílico não desenham nada (o fundo fica sólido).
pub fn windows_transparency() -> bool {
    let mut cmd = Command::new("reg");
    let out = hidden(&mut cmd)
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "EnableTransparency",
        ])
        .output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            s.to_lowercase().contains("0x1")
        }
        Err(_) => false,
    }
}

/// Liga/desliga os efeitos de transparência do Windows (só HKCU, reversível).
#[allow(dead_code)]
pub fn set_windows_transparency(enable: bool) -> Result<(), String> {
    let mut cmd = Command::new("reg");
    let out = hidden(&mut cmd)
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "EnableTransparency",
            "/t",
            "REG_DWORD",
            "/d",
            if enable { "1" } else { "0" },
            "/f",
        ])
        .output()
        .map_err(|e| format!("reg: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn startup_shortcut() -> Option<std::path::PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        std::path::PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs\Startup")
            .join(format!("{APP_NAME}.lnk")),
    )
}

fn hidden(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000) // CREATE_NO_WINDOW
}

/// Inicia o app junto com o Windows. Tenta `Run`; se a chave estiver bloqueada
/// (comum em máquinas com política de segurança), usa um atalho na pasta
/// Inicializar.
/// Nome da tarefa agendada usada para iniciar junto com o Windows **já elevado**
/// (sem o UAC piscando no login).
const TASK_NAME: &str = "Rayclip";

fn schtasks(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("schtasks");
    let out = hidden(&mut cmd)
        .args(args)
        .output()
        .map_err(|e| format!("schtasks: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// O processo está elevado (administrador)?
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// A tarefa agendada existe e está ativa?
pub fn startup_task_active() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME])
        .map(|out| !out.contains("Disabled"))
        .unwrap_or(false)
}

/// Inicia junto com o Windows.
///
/// O app tem manifesto `requireAdministrator`, e o item de `Run` **não** eleva:
/// o Windows bloquearia ou mostraria UAC a cada login. Por isso o caminho
/// preferido é uma Tarefa Agendada com `/RL HIGHEST` (dispara no login já como
/// administrador, sem prompt). `Run` e o atalho na pasta Inicializar ficam como
/// alternativa se o agendador não estiver disponível.
pub fn set_startup(enable: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let lnk = startup_shortcut();

    if !enable {
        let _ = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
        let _ = reg(&["delete", RUN_KEY, "/v", APP_NAME, "/f"]);
        if let Some(lnk) = lnk {
            let _ = std::fs::remove_file(lnk);
        }
        return Ok(());
    }

    let action = format!("\"{}\"", exe.display());
    let task = schtasks(&[
        "/Create", "/TN", TASK_NAME, "/TR", &action, "/SC", "ONLOGON", "/RL", "HIGHEST", "/F",
    ]);
    match task {
        Ok(_) => {
            // Evita abrir duas instâncias: remove os outros mecanismos.
            let _ = reg(&["delete", RUN_KEY, "/v", APP_NAME, "/f"]);
            if let Some(lnk) = lnk {
                if lnk.exists() {
                    let _ = std::fs::remove_file(lnk);
                }
            }
            Ok(())
        }
        Err(task_err) => {
            // Plano B: chave Run (sem elevação) e por último o atalho.
            let value = format!("\"{}\"", exe.display());
            if reg(&[
                "add", RUN_KEY, "/v", APP_NAME, "/t", "REG_SZ", "/d", &value, "/f",
            ])
            .is_ok()
            {
                return Ok(());
            }
            let lnk = lnk.ok_or_else(|| format!("sem pasta Inicializar ({task_err})"))?;
            let script = format!(
                "$s=(New-Object -COM WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()",
                lnk.display(),
                exe.display(),
                exe.parent().map(|p| p.display().to_string()).unwrap_or_default()
            );
            let mut cmd = Command::new("powershell");
            let out = hidden(&mut cmd)
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
                .output()
                .map_err(|e| format!("powershell: {e}"))?;
            if lnk.exists() {
                Ok(())
            } else {
                Err(format!(
                    "{task_err} / {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ))
            }
        }
    }
}
