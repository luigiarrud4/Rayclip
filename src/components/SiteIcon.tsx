import { useEffect, useState } from "react";
import type { Entry } from "../lib/types";

/** Primeira letra do domínio do link (tile sem favicon). */
export function siteLetter(entry: Entry): string {
  const raw = entry.link?.siteName || entry.text || "";
  const host = raw.replace(/^https?:\/\//, "").split(/[/?#]/)[0];
  const clean = host.replace(/^www\./, "");
  return (clean[0] || "?").toUpperCase();
}

/**
 * Favicon do site do link. Se o arquivo não existir (cache apagado) ou o
 * download falhar, cai para o tile com a inicial do domínio — nunca deixa o
 * ícone quebrado do navegador na lista.
 */
export function SiteIcon({ entry }: { entry: Entry }) {
  const url = entry.link?.icon ?? null;
  const [bad, setBad] = useState(false);

  // Entrada reciclada (mesmo componente, outro link): volta a tentar.
  useEffect(() => setBad(false), [url]);

  if (url && !bad) {
    return (
      <img
        className="favicon"
        src={url}
        alt=""
        draggable={false}
        onError={() => setBad(true)}
      />
    );
  }
  return <span className="site-tile">{siteLetter(entry)}</span>;
}
