import type { Settings } from "./types";

export type ThemePreset = {
  id: string;
  label: string;
  /** [fundo, destaque] usados no seletor */
  swatch: [string, string];
  accent: string;
};

export const THEMES: ThemePreset[] = [
  { id: "dark", label: "Raycast Dark", swatch: ["#000000", "#4fa3f8"], accent: "#4fa3f8" },
  { id: "midnight", label: "Midnight", swatch: ["#0b1224", "#7aa2ff"], accent: "#7aa2ff" },
  { id: "nord", label: "Nord", swatch: ["#2e3440", "#88c0d0"], accent: "#88c0d0" },
  { id: "rose", label: "Rosé", swatch: ["#1b1016", "#ff8fb1"], accent: "#ff8fb1" },
  { id: "light", label: "Paper Light", swatch: ["#f3f4f6", "#2563eb"], accent: "#2563eb" },
];

export const ACCENTS = [
  "#4fa3f8",
  "#a485ff",
  "#59d499",
  "#ff9217",
  "#ff6363",
  "#56c2ff",
  "#cf2f98",
  "#f5c451",
];

/** Aplica tema + cor de destaque no documento inteiro. */
// Os "efeitos de transparência" do Windows precisam estar ligados; senão o
// blur (Mica/acrílico) não desenha nada e a janela ficaria só esmaecida.
let osTransparency = true;

export function setOsTransparency(on: boolean) {
  osTransparency = on;
}

export function applyTheme(
  cfg: Pick<Settings, "theme" | "accent" | "translucent"> | null | undefined
) {
  const root = document.documentElement;
  const theme = THEMES.some((t) => t.id === cfg?.theme) ? cfg!.theme : "dark";
  root.dataset.theme = theme;
  // Translúcido (Mica) é o padrão; "off" pinta um fundo sólido.
  root.dataset.translucency = cfg?.translucent !== false && osTransparency ? "on" : "off";
  const accent = (cfg?.accent || "#4fa3f8").trim();
  root.style.setProperty("--accent", accent);
  // sombra/realce derivados do destaque escolhido
  root.style.setProperty("--accent-soft", hexAlpha(accent, 0.28));
}

function hexAlpha(hex: string, alpha: number): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return `rgb(79 163 248 / ${alpha})`;
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  return `rgb(${r} ${g} ${b} / ${alpha})`;
}
