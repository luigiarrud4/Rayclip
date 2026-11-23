//! Configurações do app: atalhos, comportamento de colagem, histórico e
//! integrações com o Windows (Win+V, iniciar com o Windows).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Atalhos usados dentro da janela (não globais). Formato: "Ctrl+K".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InAppKeys {
    pub paste: String,
    pub paste_keep_open: String,
    pub actions: String,
    pub pin: String,
    pub delete: String,
    pub clear_all: String,
    pub copy_text: String,
    pub rename: String,
    pub open_link: String,
    pub settings: String,
}

impl Default for InAppKeys {
    fn default() -> Self {
        InAppKeys {
            paste: "Enter".into(),
            paste_keep_open: "Ctrl+Enter".into(),
            actions: "Ctrl+K".into(),
            pin: "Ctrl+P".into(),
            delete: "Delete".into(),
            clear_all: "Ctrl+Shift+Delete".into(),
            copy_text: "Ctrl+T".into(),
            rename: "Ctrl+E".into(),
            open_link: "Ctrl+O".into(),
            settings: "Ctrl+,".into(),
        }
    }
}

impl InAppKeys {
    fn sanitized(mut self) -> Self {
        let def = InAppKeys::default();
        for (slot, fallback) in [
            (&mut self.paste, &def.paste),
            (&mut self.paste_keep_open, &def.paste_keep_open),
            (&mut self.actions, &def.actions),
            (&mut self.pin, &def.pin),
            (&mut self.delete, &def.delete),
            (&mut self.clear_all, &def.clear_all),
            (&mut self.copy_text, &def.copy_text),
            (&mut self.rename, &def.rename),
            (&mut self.open_link, &def.open_link),
            (&mut self.settings, &def.settings),
        ] {
            if slot.trim().is_empty() {
                *slot = fallback.clone();
            }
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Atalho global que abre/fecha o histórico.
    pub show_hotkey: String,
    /// Segundo atalho global (mesma função).
    pub paste_hotkey: String,
    /// Colar sempre sem formatação.
    pub paste_plain_default: bool,
    /// Manter a janela aberta depois de colar.
    pub keep_open_after_paste: bool,
    /// Quantas entradas guardar.
    pub max_entries: usize,
    /// Dias de retenção (0 = ilimitado).
    pub retention_days: u32,
    /// Desativar o Win+V do Windows (DisabledHotkeys) e usar o nosso atalho.
    pub replace_win_v: bool,
    /// Ignorar conteúdo vindo de gerenciadores de senha.
    pub ignore_password_managers: bool,
    /// Indexar texto de imagens (OCR).
    pub ocr_enabled: bool,
    /// Buscar título/favicon/capa dos links.
    pub link_previews: bool,
    /// Abrir junto com o Windows.
    pub launch_at_login: bool,
    /// 100% offline: não faz nenhuma requisição de rede (links ficam sem capa).
    pub offline_mode: bool,
    /// Mostrar o conteúdo da entrada ao passar o mouse (sem clicar).
    pub hover_preview: bool,
    /// Tema pronto: "dark", "light", "midnight", "nord", "rose".
    pub theme: String,
    /// Cor de destaque (hex, ex.: "#4FA3F8").
    pub accent: String,
    /// Fundo translúcido (Mica do Windows 11) — dá para ver o que está atrás.
    pub translucent: bool,
    /// Atalhos internos configuráveis.
    pub in_app_keys: InAppKeys,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_hotkey: "Alt+Space".into(),
            paste_hotkey: "Ctrl+Shift+V".into(),
            paste_plain_default: false,
            keep_open_after_paste: false,
            max_entries: crate::store::MAX_ENTRIES,
            retention_days: 0,
            replace_win_v: false,
            ignore_password_managers: true,
            ocr_enabled: true,
            link_previews: true,
            launch_at_login: false,
            offline_mode: false,
            hover_preview: true,
            theme: "dark".into(),
            accent: "#4FA3F8".into(),
            translucent: true,
            in_app_keys: InAppKeys::default(),
        }
    }
}

impl Settings {
    /// Corrige atalhos antigos salvos (o Backspace apagava entradas enquanto o
    /// usuário apagava o texto da busca).
    fn migrate(&mut self) {
        if self
            .in_app_keys
            .delete
            .eq_ignore_ascii_case("ctrl+backspace")
        {
            self.in_app_keys.delete = "Delete".into();
        }
        if self
            .in_app_keys
            .clear_all
            .eq_ignore_ascii_case("ctrl+shift+backspace")
        {
            self.in_app_keys.clear_all = "Ctrl+Shift+Delete".into();
        }
    }

    pub fn path(app: &tauri::AppHandle) -> Option<PathBuf> {
        use tauri::Manager;
        let dir = app.path().app_data_dir().ok()?;
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir.join("settings.json"))
    }

    pub fn load(app: &tauri::AppHandle) -> Self {
        let Some(path) = Self::path(app) else {
            return Settings::default();
        };
        let mut cfg: Settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        cfg.migrate();
        cfg
    }

    pub fn save(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let path = Self::path(app).ok_or("sem diretório de dados")?;
        let raw = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, raw).map_err(|e| e.to_string())
    }

    /// Normaliza valores vindos da UI (evita atalho vazio, número absurdo…).
    pub fn sanitized(mut self) -> Self {
        let def = Settings::default();
        if self.show_hotkey.trim().is_empty() {
            self.show_hotkey = def.show_hotkey;
        }
        if self.paste_hotkey.trim().is_empty() {
            self.paste_hotkey = def.paste_hotkey;
        }
        self.max_entries = self.max_entries.clamp(50, 20_000);
        self.retention_days = self.retention_days.min(3650);
        self.in_app_keys = self.in_app_keys.sanitized();
        const THEMES: [&str; 5] = ["dark", "light", "midnight", "nord", "rose"];
        if !THEMES.contains(&self.theme.as_str()) {
            self.theme = def.theme;
        }
        let a = self.accent.trim().to_string();
        let valid =
            a.len() == 7 && a.starts_with('#') && a[1..].chars().all(|c| c.is_ascii_hexdigit());
        self.accent = if valid { a } else { def.accent };
        self
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn atalhos_padrao_nao_apagam_com_backspace() {
        let k = InAppKeys::default();
        assert_eq!(k.delete, "Delete");
        assert_eq!(k.clear_all, "Ctrl+Shift+Delete");
        assert!(!k.delete.to_lowercase().contains("backspace"));
        assert!(!k.clear_all.to_lowercase().contains("backspace"));
    }

    #[test]
    fn migra_atalhos_antigos_de_backspace() {
        let mut cfg = Settings::default();
        cfg.in_app_keys.delete = "Ctrl+Backspace".into();
        cfg.in_app_keys.clear_all = "Ctrl+Shift+Backspace".into();
        cfg.migrate();
        assert_eq!(cfg.in_app_keys.delete, "Delete");
        assert_eq!(cfg.in_app_keys.clear_all, "Ctrl+Shift+Delete");
    }

    #[test]
    fn migra_so_o_que_e_antigo() {
        let mut cfg = Settings::default();
        cfg.in_app_keys.delete = "Ctrl+D".into();
        cfg.migrate();
        assert_eq!(cfg.in_app_keys.delete, "Ctrl+D");
    }
}
