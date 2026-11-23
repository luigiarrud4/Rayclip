import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { InAppKeys, Settings } from "../lib/types";
import * as api from "../api";
import { CheckIcon } from "../lib/icons";
import { Dropdown } from "./Dropdown";
import { ACCENTS, THEMES, applyTheme, setOsTransparency } from "../lib/theme";

type Section =
  | "preferences"
  | "shortcuts"
  | "paste"
  | "history"
  | "privacy"
  | "data"
  | "system"
  | "about";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "preferences", label: "Preferences" },
  { id: "shortcuts", label: "Shortcuts" },
  { id: "paste", label: "Pasting" },
  { id: "history", label: "History" },
  { id: "privacy", label: "Privacy & Indexing" },
  { id: "data", label: "Data & Security" },
  { id: "system", label: "System" },
  { id: "about", label: "About" },
];

function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      className="switch"
      data-on={checked}
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
    >
      <span className="knob" />
    </button>
  );
}

function Field({
  title,
  desc,
  children,
}: {
  title: string;
  desc?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="field">
      <div className="field-text">
        <div className="field-title">{title}</div>
        {desc && <div className="field-desc">{desc}</div>}
      </div>
      <div className="field-control">{children}</div>
    </div>
  );
}

/** Captura a próxima combinação de teclas e devolve no formato do Tauri. */
function useRecorder(onPick: (accel: string) => void) {
  const [recording, setRecording] = useState<string | null>(null);
  const ref = useRef<((e: KeyboardEvent) => void) | null>(null);

  useEffect(() => {
    document.body.dataset.recording = recording ? "1" : "";
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
      const mods: string[] = [];
      if (e.ctrlKey) mods.push("Ctrl");
      if (e.altKey) mods.push("Alt");
      if (e.shiftKey) mods.push("Shift");
      if (e.metaKey) mods.push("Super");
      const key = e.key;
      if (["Control", "Alt", "Shift", "Meta"].includes(key)) return;
      if (!mods.length) return; // exige pelo menos um modificador
      let name = key.length === 1 ? key.toUpperCase() : key;
      if (name === " ") name = "Space";
      if (name === "Escape") {
        setRecording(null);
        return;
      }
      onPick([...mods, name].join("+"));
      setRecording(null);
    };
    ref.current = handler;
    window.addEventListener("keydown", handler, true);
    return () => {
      window.removeEventListener("keydown", handler, true);
      document.body.dataset.recording = "";
    };
  }, [recording, onPick]);

  return { recording, setRecording };
}

function pretty(accel: string) {
  return accel.split("+").map((p) =>
    p === "Super" ? "Win" : p
  );
}

export function SettingsPage({
  onClose,
  onSaved,
  onChange,
}: {
  onClose: () => void;
  onSaved: (msg: string, kind?: "ok" | "err") => void;
  onChange?: (cfg: Settings) => void;
}) {
  const [section, setSection] = useState<Section>("preferences");
  const [cfg, setCfg] = useState<Settings | null>(null);
  const [sys, setSys] = useState<{
    winVDisabled: boolean;
    startupTask: boolean;
    elevated: boolean;
    exe: string;
    winTransparency?: boolean;
    encrypted?: boolean;
    dbPath?: string;
  } | null>(null);
  // Importação do histórico do Raycast.
  const [raycastCount, setRaycastCount] = useState<number | null>(null);
  const [importing, setImporting] = useState<{ done: number; total: number } | null>(null);
  const [toast, setToast] = useState<{ text: string; kind: "ok" | "err" } | null>(null);

  useEffect(() => {
    void api.raycastImportCount().then(setRaycastCount).catch(() => setRaycastCount(0));
    const un = listen<{ done: number; total: number }>("import:progress", (e) =>
      setImporting({ done: e.payload.done, total: e.payload.total })
    );
    return () => {
      void un.then((f) => f());
    };
  }, []);

  const entryOptions = useMemo(
    () => [100, 300, 500, 1000, 2000, 5000].map((n) => ({ value: n, label: `${n} entries` })),
    []
  );
  const retentionOptions = useMemo(
    () => [
      { value: 0, label: "Unlimited" },
      { value: 1, label: "1 day" },
      { value: 7, label: "1 week" },
      { value: 30, label: "1 month" },
      { value: 90, label: "3 months" },
      { value: 365, label: "1 year" },
    ],
    []
  );

  useEffect(() => {
    void api.getSettings().then(setCfg);
    void api.systemStatus().then(setSys);
  }, []);

  const update = async (patch: Partial<Settings>) => {
    if (!cfg) return;
    const next = { ...cfg, ...patch };
    setCfg(next);
    onChange?.(next);
    try {
      const warnings = await api.saveSettings(next);
      const applied = await api.getSettings();
      setCfg(applied);
      onChange?.(applied);
      setSys(await api.systemStatus());
      const msg = warnings.length
        ? `Saved with warnings: ${warnings.join(" · ")}`
        : "Settings saved";
      setToast({ text: msg, kind: warnings.length ? "err" : "ok" });
      onSaved(msg, warnings.length ? "err" : "ok");
    } catch (e) {
      const msg = `Save failed — ${e instanceof Error ? e.message : String(e)}`;
      setToast({ text: msg, kind: "err" });
      onSaved(msg, "err");
    }
  };

  const { recording, setRecording } = useRecorder((accel) => {
    if (!cfg) return;
    if (recording === "show") void update({ showHotkey: accel });
    else if (recording === "paste") void update({ pasteHotkey: accel });
    else if (recording) {
      const field = recording as keyof InAppKeys;
      void update({ inAppKeys: { ...cfg.inAppKeys, [field]: accel } });
    }
  });

  if (!cfg) {
    return (
      <div className="settings">
        <div className="settings-empty">Loading settings…</div>
      </div>
    );
  }

  const hotkeyRow = (id: "show" | "paste", value: string, title: string, desc: string) => (
    <Field title={title} desc={desc}>
      <button
        type="button"
        className="hotkey-input"
        data-recording={recording === id}
        onClick={() => setRecording(recording === id ? null : id)}
      >
        {recording === id ? "Press keys…" : pretty(value).join(" + ")}
      </button>
    </Field>
  );

  const inApp: [keyof InAppKeys, string, string][] = [
    ["paste", "Paste selected item", "Ctrl+Shift+Enter pastes as plain text"],
    ["pasteKeepOpen", "Paste and keep window open", "Great for pasting several items in a row"],
    ["actions", "Show actions panel", "Rename, pin, delete, open, OCR…"],
    ["rename", "Rename item", "Edit the name inline"],
    ["pin", "Pin / unpin item", "Pinned items stay on top"],
    ["delete", "Delete item", ""],
    ["clearAll", "Clear history (keeps pinned)", ""],
    ["copyText", "Copy text found in image", "Requires the OCR badge"],
    ["openLink", "Open link in browser", ""],
    ["settings", "Open these settings", ""],
  ];

  return (
    <div className="settings">
      <aside className="settings-nav">
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            type="button"
            className="settings-tab"
            data-active={section === s.id}
            onClick={() => setSection(s.id)}
          >
            {s.label}
          </button>
        ))}
      </aside>

      <div className="settings-body">
        {section === "preferences" && (
          <>
            <div className="settings-title">Preferences</div>

            <div className="settings-subtitle">Privacy</div>
            <Field
              title="Offline & private"
              desc="Rayclip makes no network request at all. Link entries keep just the copied URL."
            >
              <Toggle
                checked={cfg.offlineMode}
                onChange={(v) => void update({ offlineMode: v })}
              />
            </Field>
            <Field
              title="Fetch link previews online"
              desc="Reads title, favicon and cover from the site — only for links you copy"
            >
              <Toggle
                checked={cfg.linkPreviews}
                onChange={(v) => void update({ linkPreviews: v })}
              />
            </Field>
            <div className="settings-hint">
              {cfg.offlineMode ? (
                <>
                  <strong>Offline mode is on.</strong> Nothing leaves this computer: no
                  title, no favicon and no cover is downloaded, and previews already
                  cached stay as they are.
                </>
              ) : (
                <>
                  With previews on, only the domain of a copied link is contacted, once,
                  to read its page title and cover image. The result is cached on disk;
                  nothing is uploaded and no account is involved.
                </>
              )}
            </div>

            <div className="settings-subtitle">Preview</div>
            <Field
              title="Preview on hover"
              desc="Show the content of the entry under the mouse without clicking it"
            >
              <Toggle
                checked={cfg.hoverPreview}
                onChange={(v) => void update({ hoverPreview: v })}
              />
            </Field>
            <div className="settings-hint">
              With this off, use ↑ ↓ or click to move the preview. Text in the preview stays
              selectable, so you can copy just one part of a long entry.
            </div>

            <div className="settings-subtitle">Appearance</div>
            <Field title="Theme" desc="Colour scheme of the whole window">
              <div className="theme-row">
                {THEMES.map((t) => (
                  <button
                    key={t.id}
                    type="button"
                    className="theme-chip"
                    data-active={cfg.theme === t.id}
                    title={t.label}
                    onClick={() => void update({ theme: t.id, accent: t.accent })}
                  >
                    <span
                      className="theme-swatch"
                      style={{ background: t.swatch[0] }}
                    >
                      <span
                        className="theme-swatch-accent"
                        style={{ background: t.swatch[1] }}
                      />
                    </span>
                    <span className="theme-label">{t.label}</span>
                  </button>
                ))}
              </div>
            </Field>
            <Field title="Accent colour" desc="Selection, highlight and badges">
              <div className="accent-row">
                {ACCENTS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    className="accent-chip"
                    data-active={cfg.accent.toLowerCase() === c}
                    style={{ background: c }}
                    title={c}
                    onClick={() => void update({ accent: c })}
                  />
                ))}
                <input
                  className="accent-hex"
                  value={cfg.accent}
                  spellCheck={false}
                  onChange={(e) => {
                    const v = e.target.value.trim();
                    if (/^#[0-9a-fA-F]{6}$/.test(v)) void update({ accent: v });
                  }}
                  aria-label="Accent colour hex"
                />
              </div>
            </Field>
            <Field
              title="Translucent window"
              desc="Mica backdrop: you can see what is behind the window (Windows 11)"
            >
              <div className="field-col">
                <Toggle
                  checked={cfg.translucent}
                  onChange={(v) => void update({ translucent: v })}
                />
                {cfg.translucent && sys && !sys.winTransparency && (
                  <div className="hint-warn">
                    Windows transparency effects are off, so the blur can’t be drawn.{" "}
                    <button
                      type="button"
                      className="link-btn"
                      onClick={async () => {
                        try {
                          await api.setWindowsTransparency(true);
                          setOsTransparency(true);
                          applyTheme(cfg);
                          setSys(await api.systemStatus());
                          setToast({ text: "Windows transparency enabled", kind: "ok" });
                        } catch (e) {
                          setToast({ text: String(e), kind: "err" });
                        }
                      }}
                    >
                      Enable it
                    </button>
                  </div>
                )}
              </div>
            </Field>
          </>
        )}

        {section === "shortcuts" && (
          <>
            <div className="settings-title">Shortcuts</div>
            {hotkeyRow("show", cfg.showHotkey, "Open Clipboard History", "Global shortcut, works in any app")}
            {hotkeyRow("paste", cfg.pasteHotkey, "Alternative Shortcut", "Second global shortcut for the same action")}
            <div className="settings-subtitle">Global shortcuts</div>
            <div className="settings-hint">
              ↑ ↓ move the selection, Esc closes. Hold Shift while pasting to strip formatting.
            </div>
            <div className="settings-subtitle">Inside the window</div>
            {inApp.map(([field, title, desc]) => (
              <Field key={field} title={title} desc={desc}>
                <button
                  type="button"
                  className="hotkey-input"
                  data-recording={recording === field}
                  onClick={() => setRecording(recording === field ? null : field)}
                >
                  {recording === field
                    ? "Press keys…"
                    : pretty(cfg.inAppKeys[field]).join(" + ")}
                </button>
              </Field>
            ))}
            <div className="settings-hint">
              Click a shortcut and press the new combination. At least one modifier key
              (Ctrl, Alt, Shift or Win) is required.
            </div>
          </>
        )}

        {section === "paste" && (
          <>
            <div className="settings-title">Pasting</div>
            <Field title="Paste as plain text" desc="Strip formatting from every paste">
              <Toggle
                checked={cfg.pastePlainDefault}
                onChange={(v) => void update({ pastePlainDefault: v })}
              />
            </Field>
            <Field title="Keep window open after pasting" desc="Paste several entries in a row">
              <Toggle
                checked={cfg.keepOpenAfterPaste}
                onChange={(v) => void update({ keepOpenAfterPaste: v })}
              />
            </Field>
          </>
        )}

        {section === "history" && (
          <>
            <div className="settings-title">History</div>
            <Field title="Maximum entries" desc="Oldest unpinned entries are removed first">
              <Dropdown
                value={cfg.maxEntries}
                onChange={(v) => void update({ maxEntries: v })}
                options={entryOptions}
              />
            </Field>
            <Field title="Keep history for" desc="Pinned entries are never removed">
              <Dropdown
                value={cfg.retentionDays}
                onChange={(v) => void update({ retentionDays: v })}
                options={retentionOptions}
              />
            </Field>
          </>
        )}

        {section === "privacy" && (
          <>
            <div className="settings-title">Privacy & Indexing</div>
            <Field
              title="Ignore password managers"
              desc="Never record content copied from 1Password, Bitwarden, KeePass…"
            >
              <Toggle
                checked={cfg.ignorePasswordManagers}
                onChange={(v) => void update({ ignorePasswordManagers: v })}
              />
            </Field>
            <Field title="Index text in images (OCR)" desc="Search inside screenshots with Windows OCR">
              <Toggle
                checked={cfg.ocrEnabled}
                onChange={(v) => void update({ ocrEnabled: v })}
              />
            </Field>
            <div className="settings-hint">
              Everything stays on this computer. To switch off every network request, see
              <strong> Preferences → Offline &amp; private</strong>.
            </div>
          </>
        )}

        {section === "data" && (
          <>
            <div className="settings-title">Data &amp; Security</div>
            <Field
              title="Database encryption"
              desc="Clipboard text, rich text and images are stored in a SQLCipher database (AES-256 + HMAC-SHA512)"
            >
              <span className="db-badge" data-on={sys?.encrypted ? "yes" : "no"}>
                {sys?.encrypted ? "Encrypted" : "Unavailable"}
              </span>
            </Field>
            <div className="settings-hint">
              The 256-bit key lives in <code>rayclip.key</code>, sealed with Windows DPAPI, so
              only your Windows user can open it — copying the files to another account or
              computer is useless. Everything (text, HTML/RTF, images) is inside{" "}
              <code>rayclip.db</code>; no loose files are left behind.
              {sys?.dbPath && (
                <>
                  <br />
                  <span className="db-path">{sys.dbPath}</span>
                </>
              )}
            </div>
            <Field
              title="Import from Raycast"
              desc="Copies your Raycast clipboard history into this encrypted database"
            >
              <button
                type="button"
                className="btn"
                disabled={!!importing || raycastCount === 0}
                onClick={() => {
                  setImporting({ done: 0, total: raycastCount ?? 0 });
                  void api
                    .importRaycast()
                    .then((total) => {
                      setImporting(null);
                      onSaved(`Imported ${total} items from Raycast`);
                      void api.raycastImportCount().then(setRaycastCount).catch(() => {});
                    })
                    .catch((e) => {
                      setImporting(null);
                      onSaved(`Import failed — ${e}`, "err");
                    });
                }}
              >
                {importing
                  ? `Importing ${importing.done}/${importing.total || "…"}`
                  : raycastCount === null
                    ? "Check Raycast history"
                    : raycastCount === 0
                      ? "Raycast history not found"
                      : `Import ${raycastCount} items`}
              </button>
            </Field>
            <div className="settings-hint">
              Raycast keeps its own database locked with a DPAPI key, but it also writes each
              item as a file in <code>%LOCALAPPDATA%\Raycast\clipboard</code>. The import reads
              those files (images, CF_HTML, RTF, text), keeps the original dates, and leaves the
              Raycast files untouched. Raycast itself is not installed or modified.
            </div>
          </>
        )}

        {section === "system" && (
          <>
            <div className="settings-title">System</div>
            <Field
              title="Take over Win+V"
              desc="Disable the native Windows clipboard panel so Win+V opens Rayclip"
            >
              <Toggle
                checked={cfg.replaceWinV}
                onChange={(v) => void update({ replaceWinV: v })}
              />
            </Field>
            <div className="settings-hint">
              Windows keeps its own shortcut in Explorer. Rayclip writes
              <code> DisabledHotkeys = "V" </code>
              under <code>HKCU\…\Explorer\Advanced</code> (the same trick CopyQ and Ditto
              use). The change only applies after Explorer restarts.
              <br />
              Windows status:{" "}
              <strong>{sys?.winVDisabled ? "Win+V disabled" : "Win+V active in Windows"}</strong>
            </div>
            <Field title="Apply now" desc="Restarts Explorer (open File Explorer windows close)">
              <button
                type="button"
                className="btn"
                onClick={() => {
                  void api
                    .restartExplorer()
                    .then(() => onSaved("Explorer restarted"))
                    .catch((e) => onSaved(`Restart failed — ${e}`, "err"));
                }}
              >
                Restart Explorer
              </button>
            </Field>
            <Field
              title="Start with Windows"
              desc="Registers a Task Scheduler job that starts Rayclip as administrator without a UAC prompt"
            >
              <Toggle
                checked={cfg.launchAtLogin}
                onChange={(v) => void update({ launchAtLogin: v })}
              />
            </Field>
            <div className="settings-hint">
              Startup job:{" "}
              <strong>{sys?.startupTask ? "scheduled task active" : "not registered"}</strong>
              <br />
              This window:{" "}
              <strong>{sys?.elevated ? "running as administrator" : "standard user"}</strong>
              <br />
              Win+V takeover and elevated auto-start both need administrator rights, which is
              why Rayclip asks for them when it starts.
            </div>
          </>
        )}

        {section === "about" && (
          <>
            <div className="settings-title">About</div>
            <Field title="Rayclip" desc="Clipboard History for Windows, inspired by Raycast">
              <span className="about-badge">v0.1.7</span>
            </Field>
            <div className="settings-hint">
              <div>
                Executable: <code>{sys?.exe || "—"}</code>
              </div>
              <div>
                Data: <code>%APPDATA%\com.rayclip.app\clipboard</code>
              </div>
              <div className="about-line">
                <CheckIcon size={13} /> Built with Tauri + React · no account, no telemetry
              </div>
            </div>
          </>
        )}
      </div>

      {toast && (
        <div className="settings-toast" data-kind={toast.kind}>
          <span className={`status-dot ${toast.kind}`} />
          {toast.text}
        </div>
      )}
      <button type="button" className="settings-close" onClick={onClose} title="Back (Esc)">
        Esc
      </button>
    </div>
  );
}
