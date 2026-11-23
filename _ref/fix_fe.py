import re

def patch(path, pairs, count_map=None):
    s = open(path, encoding='utf-8').read()
    for a, b in pairs:
        assert a in s, f'{path}: NAO ACHOU {a[:70]}'
        s = s.replace(a, b)
    open(path, 'w', encoding='utf-8').write(s)
    print('ok', path)


# ---------------- format.ts: URL do banco cifrado ----------------
patch('src/lib/format.ts', [(
'''export const imageUrl = (path?: string | null) =>
  path ? convertFileSrc(path.replace(/\\\\/g, "/")) : "";''',
'''/** Arquivo local (ícone de app, capa de link) servido pelo protocolo de asset. */
export const fileUrl = (path?: string | null) =>
  path ? convertFileSrc(path.replace(/\\\\/g, "/")) : "";

/**
 * Imagem/miniatura que mora **dentro do banco cifrado**: o backend serve em
 * `http://rayclip.localhost/img/<id>`. `v` só serve para o navegador não
 * reaproveitar a versão antiga quando a entrada muda.
 */
export const mediaUrl = (kind: "img" | "thumb", id: string, v?: number | string) =>
  `http://rayclip.localhost/${kind}/${id}${v ? `?v=${v}` : ""}`;

/** Compatibilidade: caminho local vai pelo asset protocol. */
export const imageUrl = fileUrl;'''
)])

# ---------------- types.ts ----------------
patch('src/lib/types.ts', [
('''  paths: string[];
  imagePath?: string | null;''',
'''  paths: string[];
  /** A imagem está no banco cifrado (servida por mediaUrl). */
  hasImage?: boolean;'''),
('''  ocrText?: string | null;
  thumb?: string | null;''',
'''  ocrText?: string | null;
  /** Miniatura/poster guardado no banco (servida por mediaUrl). */
  hasThumb?: boolean;
  /** A imagem é um GIF animado. */
  gif?: boolean;'''),
])

# ---------------- EntryList ----------------
patch('src/components/EntryList.tsx', [
('import { displayTitle, imageUrl } from "../lib/format";',
 'import { displayTitle, mediaUrl } from "../lib/format";'),
('''  if (entry.kind === "image" && entry.imagePath) {
    if (/\\.gif$/i.test(entry.imagePath) && entry.thumb) return imageUrl(entry.thumb);''',
'''  if (entry.kind === "image" && entry.hasImage) {
    if (entry.gif && entry.hasThumb) return mediaUrl("thumb", entry.id, entry.createdAt);'''),
('''    return imageUrl(entry.imagePath);
  }
  if (entry.thumb) return imageUrl(entry.thumb);''',
'''    return mediaUrl("img", entry.id, entry.createdAt);
  }
  if (entry.hasThumb) return mediaUrl("thumb", entry.id, entry.createdAt);'''),
])

# ---------------- DetailsPane ----------------
patch('src/components/DetailsPane.tsx', [
('import { displayTitle, formatBytes, imageUrl, shortPath } from "../lib/format";',
 'import { displayTitle, fileUrl, formatBytes, mediaUrl, shortPath } from "../lib/format";'),
('  return path ? imageUrl(path) : null;', '  return path ? fileUrl(path) : null;'),
('''      {entry.kind === "image" && entry.imagePath && (''',
'''      {entry.kind === "image" && entry.hasImage && ('''),
('''          <img src={imageUrl(entry.imagePath)} alt="" draggable={false} />''',
'''          <img
            src={mediaUrl("img", entry.id, entry.createdAt)}
            alt=""
            draggable={false}
          />'''),
('''          {/\\.gif$/i.test(entry.imagePath) && <span className="preview-badge">GIF</span>}''',
'''          {entry.gif && <span className="preview-badge">GIF</span>}'''),
('''      {entry.kind !== "image" && !color && !isText && entry.thumb && (''',
'''      {entry.kind !== "image" && !color && !isText && entry.hasThumb && ('''),
('''          <img src={imageUrl(entry.thumb)} alt="" draggable={false} />''',
'''          <img
            src={mediaUrl("thumb", entry.id, entry.createdAt)}
            alt=""
            draggable={false}
          />'''),
('''            v={entry.imagePath && /\\.gif$/i.test(entry.imagePath) ? "GIF Image" : "PNG Image"}''',
'''            v={entry.gif ? "GIF Image" : "PNG Image"}'''),
])

# ---------------- SaveManyDialog ----------------
patch('src/components/SaveManyDialog.tsx', [
('  return !!e.imagePath;', '  return !!e.hasImage;'),
('  if (isImage(e)) return e.imagePath?.toLowerCase().endsWith(".gif") ? "gif" : "png";',
 '  if (isImage(e)) return e.gif ? "gif" : "png";'),
])

# ---------------- App.tsx ----------------
patch('src/App.tsx', [
('''    if (!sel || sel.thumb) return;''', '''    if (!sel || sel.hasThumb) return;'''),
('''      const hasContent =
        !!selected.imagePath || !!selected.text || selected.paths.length > 0;''',
'''      const hasContent =
        !!selected.hasImage || !!selected.text || selected.paths.length > 0;'''),
('''      const isImg = !!selected.imagePath;''', '''      const isImg = !!selected.hasImage;'''),
('''    const saveable =
      !!selected.imagePath || !!selected.text || selected.paths.length > 0;''',
'''    const saveable =
      !!selected.hasImage || !!selected.text || selected.paths.length > 0;'''),
])
