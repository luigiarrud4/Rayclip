"""Mutações devolvem só um resumo (removed/total), não o histórico inteiro.

Antes, cada clique em "fixar", renomear, editar ou apagar devolvia as 1.683
entradas (1,8 MB) pela IPC — a interface usava isso para se atualizar e agora
pede a página de novo, então o payload era só custo.
"""

import io

P = "C:/Users/Lugia/Projects/rayclip/src-tauri/src/lib.rs"
s = io.open(P, encoding="utf-8").read()

# 1) tipo de resposta + helper
old = '''/// Renomeia uma entrada (título escolhido pelo usuário). O título só é'''
new = '''/// Resposta das mutações (fixar, renomear, editar, apagar).
///
/// Antes cada uma devolvia o histórico inteiro — 1,8 MB por clique. A interface
/// não usa mais esse payload (ela pede a página de novo, §8.44); aqui vai só o
/// que o aviso da tela precisa.
#[derive(serde::Serialize)]
struct Mudanca {
    /// Quantas entradas saíram (0 quando a ação não apagou nada).
    removed: usize,
    /// Quantas continuam no histórico.
    total: usize,
}

fn mudanca(state: &AppState, removed: usize) -> Mudanca {
    let total = lock_store(&state.store).entries.len();
    Mudanca { removed, total }
}

/// Renomeia uma entrada (título escolhido pelo usuário). O título só é'''
assert old in s
s = s.replace(old, new, 1)

# 2) rename_entry
old = '''fn rename_entry(_app: AppHandle, state: State<AppState>, id: String, title: String) -> Vec<Entry> {'''
new = '''fn rename_entry(_app: AppHandle, state: State<AppState>, id: String, title: String) -> Mudanca {'''
assert old in s
s = s.replace(old, new, 1)
old = '''    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}

#[tauri::command]
fn toggle_pin(_app: AppHandle, state: State<AppState>, id: String) -> Vec<Entry> {
    {
        let mut store = lock_store(&state.store);
        store.toggle_pin(&id);
    }
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}'''
new = '''    mudanca(&state, 0)
}

#[tauri::command]
fn toggle_pin(_app: AppHandle, state: State<AppState>, id: String) -> Mudanca {
    {
        let mut store = lock_store(&state.store);
        store.toggle_pin(&id);
    }
    mudanca(&state, 0)
}'''
assert old in s
s = s.replace(old, new, 1)

# 3) move_pinned
old = '''fn move_pinned(_app: AppHandle, state: State<AppState>, id: String, up: bool) -> Vec<Entry> {'''
new = '''fn move_pinned(_app: AppHandle, state: State<AppState>, id: String, up: bool) -> Mudanca {'''
assert old in s
s = s.replace(old, new, 1)
old = '''            log(&format!("move_pinned {id} ignorado (nao fixado ou no limite)"));
        }
    }
    entries_now(&state)
}'''
new = '''            log(&format!("move_pinned {id} ignorado (nao fixado ou no limite)"));
        }
    }
    mudanca(&state, 0)
}'''
assert old in s
s = s.replace(old, new, 1)

# 4) update_entry_content
old = '''fn update_entry_content(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    text: String,
) -> Vec<Entry> {'''
new = '''fn update_entry_content(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    text: String,
) -> Mudanca {'''
assert old in s
s = s.replace(old, new, 1)
old = '''    notify_changed(&app);
    entries_now(&state)
}'''
new = '''    notify_changed(&app);
    mudanca(&state, 0)
}'''
assert old in s
s = s.replace(old, new, 1)

# 5) delete_recent
old = '''fn delete_recent(_app: AppHandle, state: State<AppState>, minutes: u32) -> Vec<Entry> {'''
new = '''fn delete_recent(_app: AppHandle, state: State<AppState>, minutes: u32) -> Mudanca {'''
assert old in s
s = s.replace(old, new, 1)
old = '''        log(&format!("delete_recent {minutes}min -> {} entradas", doomed.len()));
        for id in &doomed {
            store.remove(id);
        }
    }
    entries_now(&state)
}'''
new = '''        log(&format!("delete_recent {minutes}min -> {} entradas", doomed.len()));
        for id in &doomed {
            store.remove(id);
        }
        return mudanca(&state, doomed.len());
    }
}'''
assert old in s
s = s.replace(old, new, 1)

# 6) delete_entry
old = '''fn delete_entry(_app: AppHandle, state: State<AppState>, id: String) -> Vec<Entry> {
    log(&format!("delete_entry id={id}"));
    {
        let mut store = lock_store(&state.store);
        store.remove(&id);
    }
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}'''
new = '''fn delete_entry(_app: AppHandle, state: State<AppState>, id: String) -> Mudanca {
    log(&format!("delete_entry id={id}"));
    {
        let mut store = lock_store(&state.store);
        let existia = store.entries.iter().any(|e| e.id == id);
        store.remove(&id);
        return mudanca(&state, if existia { 1 } else { 0 });
    }
}'''
assert old in s
s = s.replace(old, new, 1)

# 7) delete_entries
old = '''fn delete_entries(_app: AppHandle, state: State<AppState>, ids: Vec<String>) -> Vec<Entry> {
    log(&format!("delete_entries n={}", ids.len()));
    {
        let mut store = lock_store(&state.store);
        for id in &ids {
            store.remove(id);
        }
    }
    entries_now(&state)
}'''
new = '''fn delete_entries(_app: AppHandle, state: State<AppState>, ids: Vec<String>) -> Mudanca {
    log(&format!("delete_entries n={}", ids.len()));
    {
        let mut store = lock_store(&state.store);
        let antes = store.entries.len();
        for id in &ids {
            store.remove(id);
        }
        let saiu = antes - store.entries.len();
        return mudanca(&state, saiu);
    }
}'''
assert old in s
s = s.replace(old, new, 1)

# 8) clear_entries
old = '''fn clear_entries(_app: AppHandle, state: State<AppState>, keep_pinned: bool) -> Vec<Entry> {'''
new = '''fn clear_entries(_app: AppHandle, state: State<AppState>, keep_pinned: bool) -> Mudanca {'''
assert old in s
s = s.replace(old, new, 1)
old = '''        let kept: Vec<Entry> = store
            .entries
            .drain(..)
            .filter(|e| keep_pinned && e.pinned)
            .collect();
        store.entries = kept;
        store.save();
    }
    // A UI que chamou já recebe a lista: sem evento (evitava 2 listagens).
    entries_now(&state)
}'''
new = '''        let antes = store.entries.len();
        let kept: Vec<Entry> = store
            .entries
            .drain(..)
            .filter(|e| keep_pinned && e.pinned)
            .collect();
        store.entries = kept;
        store.save();
        let saiu = antes - store.entries.len();
        return mudanca(&state, saiu);
    }
}'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("lib.rs: mutações devolvem Mudanca")
