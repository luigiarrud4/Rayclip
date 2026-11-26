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

/**
 * Sugestões de fundo. Não é uma lista fechada: o campo hex aceita qualquer
 * `#RRGGBB` (o seletor nativo de cor abriria um diálogo do Windows, e a janela
 * se esconde ao perder o foco — daí os chips).
 */
export const BACKGROUNDS = [
  "#000000",
  "#0a0a0c",
  "#14161c",
  "#1b1016",
  "#121b2e",
  "#1f2430",
  "#2b2f3a",
  "#3a3f4b",
];

/** Aplica tema + cor de destaque no documento inteiro. */
// Os "efeitos de transparência" do Windows precisam estar ligados; senão o
// blur (Mica/acrílico) não desenha nada e a janela ficaria só esmaecida.
let osTransparency = true;

export function setOsTransparency(on: boolean) {
  osTransparency = on;
}

export function applyTheme(
  cfg:
    | Pick<Settings, "theme" | "accent" | "translucent" | "bgColor" | "bgOpacity">
    | null
    | undefined
) {
  const root = document.documentElement;
  const theme = THEMES.some((t) => t.id === cfg?.theme) ? cfg!.theme : "dark";
  root.dataset.theme = theme;
  // Translúcido (Mica) é o padrão; "off" pinta um fundo sólido.
  root.dataset.translucency = cfg?.translucent !== false && osTransparency ? "on" : "off";
  aplicarFundo(
    root,
    cfg?.bgColor,
    cfg?.bgOpacity,
    cfg?.translucent !== false && osTransparency
  );
  const accent = (cfg?.accent || "#4fa3f8").trim();
  root.style.setProperty("--accent", accent);
  // sombra/realce derivados do destaque escolhido
  root.style.setProperty("--accent-soft", hexAlpha(accent, 0.28));
}

/**
 * Fundo escolhido à mão: cor e transparência.
 *
 * O valor entra **inline** no `<html>` porque o CSS define `--window-bg` por
 * tema e por "translúcido"; o inline vence os dois. Com o padrão (sem cor e
 * 100%) as propriedades saem e a aparência é exatamente a do tema.
 */
function aplicarFundo(
  root: HTMLElement,
  cor: string | undefined,
  opacidade: number | undefined,
  translucido: boolean
) {
  root.style.removeProperty("--window-bg");
  root.style.removeProperty("--page-bg");
  // Sem o desfoque ligado não existe transparência: a janela vira um vidro
  // sem blur (só o papel de parede cru aparecendo), que é feio e ilegível.
  const alfa = translucido ? (typeof opacidade === "number" && opacidade > 0 ? opacidade : 1) : 1;
  const corLimpa = (cor || "").trim();
  // Com o desfoque ligado o alfa vem **sempre** do controle do usuário: se ele
  // põe 100%, a janela é sólida de verdade (antes caía no 0,55 do tema e o
  // controle não fazia sentido no fim da escala).
  if (!(corLimpa.length === 7 || translucido)) return;
  const estilo = getComputedStyle(root);
  const base = rgbDe(corLimpa) ||
    rgbDe(estilo.getPropertyValue("--window-bg")) || { r: 10, g: 10, b: 12 };
  root.style.setProperty("--window-bg", `rgb(${base.r} ${base.g} ${base.b} / ${alfa})`);
  // A página de Preferências usa `--page-bg`: sem ela o interior da janela
  // continuaria opaco e a transparência só apareceria no que está por baixo.
  const painel = rgbDe(estilo.getPropertyValue("--page-bg")) || base;
  root.style.setProperty(
    "--page-bg",
    `rgb(${painel.r} ${painel.g} ${painel.b} / ${Math.min(1, alfa + 0.08)})`
  );
}

/** `#rrggbb`, `rgb(r g b)` ou `rgb(r g b / a)` -> componentes. */
function rgbDe(valor: string): { r: number; g: number; b: number } | null {
  const v = (valor || "").trim();
  const hex = /^#([0-9a-f]{6})$/i.exec(v);
  if (hex) {
    const n = parseInt(hex[1], 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }
  const m = /^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i.exec(v);
  if (m) return { r: +m[1], g: +m[2], b: +m[3] };
  return null;
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
