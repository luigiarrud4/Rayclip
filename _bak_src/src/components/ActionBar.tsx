import { Clipboard } from "../lib/icons";
import type { Status } from "../lib/types";

export function ActionBar({
  onPaste,
  onActions,
  hint,
  status,
  onSettings,
}: {
  onPaste: () => void;
  onActions: () => void;
  onSettings: () => void;
  hint?: string | null;
  status?: Status | null;
}) {
  const message = status?.text ?? hint ?? "Clipboard History";
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
          title="Settings"
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
        <div className="ab-group interactive" onClick={onPaste}>
          <span>Paste</span>
          <span className="keycap">↵</span>
        </div>
        <div className="ab-sep" />
        <div className="ab-group interactive" onClick={onActions}>
          <span>Actions</span>
          <span className="keycap">Ctrl</span>
          <span className="keycap">K</span>
        </div>
      </div>
    </footer>
  );
}
