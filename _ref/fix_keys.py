import re

# ---------------------------------------------------------------- 1) atalhos
p = 'src/lib/types.ts'
s = open(p, encoding='utf-8').read()
s = s.replace('''  delete: "Ctrl+Backspace",
  clearAll: "Ctrl+Shift+Backspace",''', '''  delete: "Delete",
  clearAll: "Ctrl+Shift+Delete",''')
open(p, 'w', encoding='utf-8').write(s)

p = 'src-tauri/src/settings.rs'
s = open(p, encoding='utf-8').read()
s = s.replace('''            delete: "Ctrl+Backspace".into(),
            clear_all: "Ctrl+Shift+Backspace".into(),''', '''            delete: "Delete".into(),
            clear_all: "Ctrl+Shift+Delete".into(),''')
# migração: quem já tinha Ctrl+Backspace salvo passa a usar Delete
old = '''impl Settings {
    pub fn path(app: &tauri::AppHandle) -> Option<PathBuf> {'''
new = '''impl Settings {
    /// Corrige atalhos antigos salvos (o Backspace apagava entradas enquanto o
    /// usuário apagava o texto da busca).
    fn migrate(&mut self) {
        if self.in_app_keys.delete.eq_ignore_ascii_case("ctrl+backspace") {
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

    pub fn path(app: &tauri::AppHandle) -> Option<PathBuf> {'''
assert old in s
s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8').write(s)
print('atalhos ok (defaults + migrate)')
