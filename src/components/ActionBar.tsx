import { Clipboard } from "../lib/icons";
import type { InAppKeys, Status } from "../lib/types";
import { DEFAULT_IN_APP_KEYS } from "../lib/types";
import { keycaps } from "../lib/keys";
import type { T } from "../lib/i18n";

export function ActionBar({
  onPaste,
  onActions,
  hint,
  status,
  onSettings,
  keys,
  updateAvailable,
  onUpdate,
  t,
}: {
  onPaste: () => void;
  onActions: () => void;
  onSettings: () => void;
  hint?: string | null;
  status?: Status | null;
  keys?: InAppKeys;
  /** Versão nova encontrada (ex.: "0.1.24"); null = nada ao rodapé. */
  updateAvailable?: string | null;
  onUpdate?: () => void;
  t: T;
}) {
  const K = keys ?? DEFAULT_IN_APP_KEYS;
  const cap = (k: string) => (k === "Enter" ? "\u21b5" : k);
  const message = status?.text ?? hint ?? t("Clipboard History");
  return (
    <footer className="actionbar" data-tauri-drag-region>
      <div
        className="actionbar-left"
        data-tauri-drag-region
        data-state={status?.kind ?? "idle"}
      >
        <button
          type="button"
          className="glyph settings-trigger"
          data-no-drag
          title={t("Settings")}
          onClick={(e) => {
            e.stopPropagation();
            onSettings();
          }}
        >
          {status ? <span className={`status-dot ${status.kind}`} /> : <Clipboard size={14} />}
        </button>
        <span className="status-text" data-tauri-drag-region>
          {message}
        </span>
      </div>
      <div className="actionbar-right">
        {updateAvailable && (
          <>
            <button
              type="button"
              className="update-pill"
              data-no-drag
              title={t("Update available — click to install")}
              onClick={(e) => {
                e.stopPropagation();
                onUpdate?.();
              }}
            >
              <span className="update-dot" />
              {t("Update {v}", { v: updateAvailable })}
            </button>
            <div className="ab-sep" />
          </>
        )}
        <div className="ab-group interactive" onClick={onPaste}>
          <span>{t("Paste")}</span>
          {keycaps(K.paste).map((k) => (
            <span className="keycap" key={k}>
              {cap(k)}
            </span>
          ))}
        </div>
        <div className="ab-sep" />
        <div className="ab-group interactive" onClick={onActions}>
          <span>{t("Actions")}</span>
          {keycaps(K.actions).map((k) => (
            <span className="keycap" key={k}>
              {cap(k)}
            </span>
          ))}
        </div>
      </div>
    </footer>
  );
}
