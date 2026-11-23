/** Compara um KeyboardEvent com um acelerador tipo "Ctrl+Shift+V". */
export function matchAccel(e: KeyboardEvent, accel: string | undefined): boolean {
  if (!accel) return false;
  const parts = accel.split("+").map((p) => p.trim());
  const key = parts[parts.length - 1];
  const mods = new Set(parts.slice(0, -1).map((m) => m.toLowerCase()));
  if (e.ctrlKey !== mods.has("ctrl")) return false;
  if (e.altKey !== mods.has("alt")) return false;
  if (e.shiftKey !== mods.has("shift")) return false;
  if (e.metaKey !== (mods.has("super") || mods.has("meta"))) return false;
  const norm = (k: string) => (k.length === 1 ? k.toUpperCase() : k.toLowerCase());
  return norm(e.key) === norm(key);
}

/** Acelerador -> keycaps curtos para o painel de acoes. */
export function keycaps(accel: string | undefined): string[] {
  if (!accel) return [];
  return accel.split("+").map((part) => {
    const p = part.trim();
    const low = p.toLowerCase();
    if (low === "ctrl" || low === "control") return "Ctrl";
    if (low === "shift") return "Shift";
    if (low === "alt") return "Alt";
    if (low === "super" || low === "win" || low === "meta") return "Win";
    if (low === "enter" || low === "return") return "Enter";
    if (low === "backspace") return "Backspace";
    if (low === "escape") return "Esc";
    if (low === "space") return "Space";
    return p.toUpperCase();
  });
}
