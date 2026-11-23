/**
 * "Copy Color as…" do Raycast: a partir do texto de uma entrada de cor,
 * devolve as versões prontas para colar (HEX, RGB, HSL, HSV…).
 *
 * O parse cobre o que o clipboard costuma trazer: `#rgb`, `#rrggbb`,
 * `#rrggbbaa`, `rgb()/rgba()` e `hsl()/hsla()`.
 */

export type ColorFmt = { id: string; label: string; value: string };

type RGBA = { r: number; g: number; b: number; a: number };

const clamp255 = (v: number) => Math.max(0, Math.min(255, Math.round(v)));

function fromHsl(h: number, s: number, l: number, a: number): RGBA {
  const hh = ((h % 360) + 360) % 360;
  const ss = Math.max(0, Math.min(1, s));
  const ll = Math.max(0, Math.min(1, l));
  const c = (1 - Math.abs(2 * ll - 1)) * ss;
  const x = c * (1 - Math.abs(((hh / 60) % 2) - 1));
  const m = ll - c / 2;
  const seg = Math.floor(hh / 60) % 6;
  const rgb: [number, number, number] =
    seg === 0
      ? [c, x, 0]
      : seg === 1
        ? [x, c, 0]
        : seg === 2
          ? [0, c, x]
          : seg === 3
            ? [0, x, c]
            : seg === 4
              ? [x, 0, c]
              : [c, 0, x];
  return {
    r: clamp255((rgb[0] + m) * 255),
    g: clamp255((rgb[1] + m) * 255),
    b: clamp255((rgb[2] + m) * 255),
    a,
  };
}

/** Aceita um número solto ou uma porcentagem ("50%"). */
function num(part: string, scale = 1): number {
  const v = parseFloat(part);
  if (Number.isNaN(v)) return 0;
  return part.includes("%") ? (v / 100) * scale : v;
}

export function parseColor(input: string): RGBA | null {
  const s = input.trim().toLowerCase();
  const hex = s.match(/^#([0-9a-f]{3,8})$/);
  if (hex) {
    let h = hex[1];
    if (h.length === 3 || h.length === 4) h = [...h].map((c) => c + c).join("");
    if (h.length !== 6 && h.length !== 8) return null;
    return {
      r: parseInt(h.slice(0, 2), 16),
      g: parseInt(h.slice(2, 4), 16),
      b: parseInt(h.slice(4, 6), 16),
      a: h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1,
    };
  }
  const fn = s.match(/^(rgba?|hsla?)\(([^)]+)\)$/);
  if (fn) {
    const parts = fn[2].split(/[\s,/]+/).filter(Boolean);
    if (parts.length < 3) return null;
    if (fn[1].startsWith("rgb")) {
      return {
        r: clamp255(num(parts[0], 255)),
        g: clamp255(num(parts[1], 255)),
        b: clamp255(num(parts[2], 255)),
        a: parts[3] !== undefined ? Math.max(0, Math.min(1, num(parts[3], 1))) : 1,
      };
    }
    return fromHsl(
      parseFloat(parts[0]),
      num(parts[1], 1),
      num(parts[2], 1),
      parts[3] !== undefined ? Math.max(0, Math.min(1, num(parts[3], 1))) : 1
    );
  }
  return null;
}

function toHsl({ r, g, b }: RGBA): [number, number, number] {
  const rr = r / 255;
  const gg = g / 255;
  const bb = b / 255;
  const max = Math.max(rr, gg, bb);
  const min = Math.min(rr, gg, bb);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return [0, 0, l];
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === rr) h = ((gg - bb) / d) % 6;
  else if (max === gg) h = (bb - rr) / d + 2;
  else h = (rr - gg) / d + 4;
  h *= 60;
  if (h < 0) h += 360;
  return [h, s, l];
}

function toHsv({ r, g, b }: RGBA): [number, number, number] {
  const [h, s, l] = toHsl({ r, g, b, a: 1 });
  const v = l + s * Math.min(l, 1 - l);
  const sv = v === 0 ? 0 : 2 * (1 - l / v);
  return [h, sv, v];
}

const round = (v: number) => Math.round(v * 100) / 100;

/** Formatos na ordem em que o usuário costuma querer. */
export function colorFormats(text: string): ColorFmt[] {
  const c = parseColor(text);
  if (!c) return [];
  const hex =
    "#" +
    [c.r, c.g, c.b]
      .map((v) => clamp255(v).toString(16).padStart(2, "0"))
      .join("")
      .toUpperCase();
  const alphaHex = clamp255(c.a * 255)
    .toString(16)
    .padStart(2, "0")
    .toUpperCase();
  const [h, s, l] = toHsl(c);
  const [hv, sv, vv] = toHsv(c);
  const a = round(c.a);
  const list: ColorFmt[] = [{ id: "hex", label: "HEX", value: hex }];
  if (c.a < 1) list.push({ id: "hex8", label: "HEX + Alpha", value: hex + alphaHex });
  list.push({ id: "rgb", label: "RGB", value: `rgb(${c.r}, ${c.g}, ${c.b})` });
  if (c.a < 1) list.push({ id: "rgba", label: "RGBA", value: `rgba(${c.r}, ${c.g}, ${c.b}, ${a})` });
  list.push({
    id: "hsl",
    label: "HSL",
    value: `hsl(${Math.round(h)}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%)`,
  });
  list.push({
    id: "hsv",
    label: "HSV",
    value: `hsv(${Math.round(hv)}, ${Math.round(sv * 100)}%, ${Math.round(vv * 100)}%)`,
  });
  return list;
}
