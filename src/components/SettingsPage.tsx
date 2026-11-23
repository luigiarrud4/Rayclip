import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { InAppKeys, Settings, UpdateInfo } from "../lib/types";
import * as api from "../api";
import { CheckIcon } from "../lib/icons";
import { Dropdown } from "./Dropdown";
import { ACCENTS, THEMES, applyTheme, setOsTransparency } from "../lib/theme";
import { LANGUAGES, type Lang, type T } from "../lib/i18n";

type Section =
  | "preferences"
  | "shortcuts"
  | "paste"
  | "history"
  | "privacy"
  | "data"
  | "system"
  | "updates"
  | "about";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "preferences", label: "Preferences" },
  { id: "shortcuts", label: "Shortcuts" },
  { id: "paste", label: "Pasting" },
  { id: "history", label: "History" },
  { id: "privacy", label: "Privacy & Indexing" },
  { id: "data", label: "Data & Security" },
  { id: "system", label: "System" },
  { id: "updates", label: "Updates" },
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
      // Esc cancela ANTES da exigencia de modificador: antes ele caia no
      // `return` e nao havia como sair da gravacao pelo teclado.
      if (key === "Escape") {
        setRecording(null);
        return;
      }
      let name = key.length === 1 ? key.toUpperCase() : key;
      if (name === " ") name = "Space";
      // Atalhos GLOBAIS (sistema) precisam de modificador. Os de dentro do app
      // aceitam tecla solta: o padrao e "Enter" para colar e "Delete" para
      // apagar, e com esta regra nao dava para voltar a eles pelo gravador.
      if (!mods.length) {
        if (recording === "g-show" || recording === "g-paste") return;
        const bare = /^(F\d{1,2}|Arrow\w+|Enter|Delete|Backspace|Tab|Home|End|PageUp|PageDown)$/.test(
          name,
        );
        if (!bare) return;
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
  t,
  startSection,
  availableUpdate,
  onUpdateFound,
}: {
  onClose: () => void;
  onSaved: (msg: string, kind?: "ok" | "err") => void;
  onChange?: (cfg: Settings) => void;
  t: T;
  startSection?: Section;
  /** Release mais nova já encontrada em segundo plano. */
  availableUpdate?: UpdateInfo | null;
  onUpdateFound?: (info: UpdateInfo | null) => void;
}) {
  const [section, setSection] = useState<Section>(startSection ?? "preferences");
  const [cfg, setCfg] = useState<Settings | null>(null);
  const [sys, setSys] = useState<{
    version?: string;
    winVDisabled: boolean;
    startupTask: boolean;
    elevated: boolean;
    exe: string;
    winTransparency?: boolean;
    encrypted?: boolean;
    keyError?: string | null;
    dbPath?: string;
  } | null>(null);
  // Importação do histórico do Raycast.
  const [raycastCount, setRaycastCount] = useState<number | null>(null);
  const [importing, setImporting] = useState<{ done: number; total: number } | null>(null);
  const [toast, setToast] = useState<{ text: string; kind: "ok" | "err" } | null>(null);
  // Atualização: a release já encontrada em segundo plano chega por prop.
  const [release, setRelease] = useState<UpdateInfo | null>(availableUpdate ?? null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<{ downloaded: number; total: number } | null>(null);
  const [checkMsg, setCheckMsg] = useState<{ text: string; kind: "ok" | "err" } | null>(null);

  useEffect(() => {
    if (availableUpdate) setRelease(availableUpdate);
  }, [availableUpdate]);

  useEffect(() => {
    const un = listen<{ downloaded: number; total: number }>("update:progress", (e) =>
      setProgress(e.payload)
    );
    return () => {
      void un.then((f) => f());
    };
  }, []);

  const checkNow = async () => {
    setChecking(true);
    setCheckMsg(null);
    try {
      const info = await api.updateCheck(true);
      setRelease(info);
      onUpdateFound?.(info);
      setCheckMsg({
        text: info
          ? t("Version {v} is available", { v: info.version })
          : t("You are on the latest version"),
        kind: "ok",
      });
    } catch (e) {
      setCheckMsg({
        text: t("Could not check for updates — {msg}", {
          msg: e instanceof Error ? e.message : String(e),
        }),
        kind: "err",
      });
    } finally {
      setChecking(false);
    }
  };

  const installNow = async () => {
    setInstalling(true);
    setProgress({ downloaded: 0, total: release?.size ?? 0 });
    setCheckMsg(null);
    try {
      const v = await api.updateInstall();
      // O app fecha logo em seguida para o exe novo assumir o banco.
      setCheckMsg({ text: t("Installed v{v} — Rayclip is reopening", { v }), kind: "ok" });
    } catch (e) {
      setInstalling(false);
      setProgress(null);
      setCheckMsg({
        text: t("Update failed — {msg}", {
          msg: e instanceof Error ? e.message : String(e),
        }),
        kind: "err",
      });
    }
  };

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
    () =>
      [100, 300, 500, 1000, 2000, 5000].map((n) => ({
        value: n,
        label: t("{n} entries", { n: n.toLocaleString() }),
      })),
    [t]
  );
  const retentionOptions = useMemo(
    () => [
      { value: 0, label: t("Unlimited") },
      { value: 1, label: t("1 day") },
      { value: 7, label: t("1 week") },
      { value: 30, label: t("1 month") },
      { value: 90, label: t("3 months") },
      { value: 365, label: t("1 year") },
    ],
    [t]
  );

  useEffect(() => {
    void api.getSettings().then(setCfg);
    void api.systemStatus().then(setSys);
  }, []);

  // O aviso nunca sumia sozinho: "Settings saved" ficava por cima do painel
  // para sempre (e empilhava um por clique). Some depois de alguns segundos.
  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), toast.kind === "err" ? 6000 : 2400);
    return () => window.clearTimeout(t);
  }, [toast]);

  // Cada clique inicia um save; a resposta do save anterior pode chegar
  // depois e regravar o estado velho (o toggle "voltava" sozinho).
  const saveSeq = useRef(0);

  const update = async (patch: Partial<Settings>) => {
    if (!cfg) return;
    const seq = ++saveSeq.current;
    const next = { ...cfg, ...patch };
    setCfg(next);
    onChange?.(next);
    try {
      const warnings = await api.saveSettings(next);
      if (seq !== saveSeq.current) return;
      const applied = await api.getSettings();
      if (seq !== saveSeq.current) return;
      setCfg(applied);
      onChange?.(applied);
      // force: o cache do backend tem 30 s e aqui o valor acabou de mudar.
      const status = await api.systemStatus(true);
      if (seq !== saveSeq.current) return;
      setSys(status);
      const msg = warnings.length
        ? t("Saved with warnings: {list}", { list: warnings.join(" · ") })
        : t("Settings saved");
      setToast({ text: msg, kind: warnings.length ? "err" : "ok" });
      onSaved(msg, warnings.length ? "err" : "ok");
    } catch (e) {
      if (seq !== saveSeq.current) return;
      const msg = t("Save failed — {msg}", {
        msg: e instanceof Error ? e.message : String(e),
      });
      setToast({ text: msg, kind: "err" });
      onSaved(msg, "err");
    }
  };

  const { recording, setRecording } = useRecorder((accel) => {
    if (!cfg) return;
    // Globais tem id proprio ("g-*"); os internos vem como "inapp:<campo>".
    // Antes o interno "paste" casava com o global e gravava no lugar errado.
    if (recording === "g-show") void update({ showHotkey: accel });
    else if (recording === "g-paste") void update({ pasteHotkey: accel });
    else if (recording?.startsWith("inapp:")) {
      const field = recording.slice("inapp:".length) as keyof InAppKeys;
      void update({ inAppKeys: { ...cfg.inAppKeys, [field]: accel } });
    }
  });

  if (!cfg) {
    return (
      <div className="settings">
        <div className="settings-empty">{t("Loading settings…")}</div>
      </div>
    );
  }

  const hotkeyRow = (id: "g-show" | "g-paste", value: string, title: string, desc: string) => (
    <Field title={title} desc={desc}>
      <button
        type="button"
        className="hotkey-input"
        data-recording={recording === id}
        onClick={() => setRecording(recording === id ? null : id)}
      >
        {recording === id ? t("Press keys…") : pretty(value).join(" + ")}
      </button>
    </Field>
  );

  const inApp: [keyof InAppKeys, string, string][] = [
    ["paste", "Paste selected item", "Hold Shift while pasting to strip formatting"],
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
            {t(s.label)}
          </button>
        ))}
      </aside>

      <div className="settings-body">
        {section === "preferences" && (
          <>
            <div className="settings-title">{t("Preferences")}</div>

            <div className="settings-subtitle">{t("Language")}</div>
            <Field title={t("Language")} desc={t("Language of the whole interface")}>
              <Dropdown<string>
                value={cfg.language ?? "en"}
                width={200}
                options={LANGUAGES.map((l) => ({ value: l.id, label: l.label }))}
                onChange={(v) =>
                  void update({ language: v as Lang })
                }
              />
            </Field>

            <div className="settings-subtitle">{t("Privacy")}</div>
            <Field
              title={t("Offline & private")}
              desc={t(
                "Rayclip makes no network request at all. Link entries keep just the copied URL."
              )}
            >
              <Toggle
                checked={cfg.offlineMode}
                onChange={(v) => void update({ offlineMode: v })}
              />
            </Field>
            <Field
              title={t("Fetch link previews online")}
              desc={t("Reads title, favicon and cover from the site — only for links you copy")}
            >
              <Toggle
                checked={cfg.linkPreviews}
                onChange={(v) => void update({ linkPreviews: v })}
              />
            </Field>
            <div className="settings-hint">
              {cfg.offlineMode
                ? t(
                    "Offline mode is on. Nothing leaves this computer: no title, no favicon and no cover is downloaded, and previews already cached stay as they are."
                  )
                : t(
                    "With previews on, only the domain of a copied link is contacted, once, to read its page title and cover image. The result is cached on disk; nothing is uploaded and no account is involved."
                  )}
            </div>

            <div className="settings-subtitle">{t("Preview")}</div>
            <Field
              title={t("Preview on hover")}
              desc={t("Show the content of the entry under the mouse without clicking it")}
            >
              <Toggle
                checked={cfg.hoverPreview}
                onChange={(v) => void update({ hoverPreview: v })}
              />
            </Field>
            <div className="settings-hint">
              {t(
                "With this off, use ↑ ↓ or click to move the preview. Text in the preview stays selectable, so you can copy just one part of a long entry."
              )}
            </div>

            <div className="settings-subtitle">{t("Appearance")}</div>
            <Field title={t("Theme")} desc={t("Colour scheme of the whole window")}>
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
            <Field title={t("Accent colour")} desc={t("Selection, highlight and badges")}>
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
                  aria-label={t("Accent colour hex")}
                />
              </div>
            </Field>
            <Field
              title={t("Translucent window")}
              desc={t("Mica backdrop: you can see what is behind the window (Windows 11)")}
            >
              <div className="field-col">
                <Toggle
                  checked={cfg.translucent}
                  onChange={(v) => void update({ translucent: v })}
                />
                {cfg.translucent && sys && !sys.winTransparency && (
                  <div className="hint-warn">
                    {t("Windows transparency effects are off, so the blur can’t be drawn.")}{" "}
                    <button
                      type="button"
                      className="link-btn"
                      onClick={async () => {
                        try {
                          await api.setWindowsTransparency(true);
                          setOsTransparency(true);
                          applyTheme(cfg);
                          setSys(await api.systemStatus(true));
                          setToast({ text: t("Windows transparency enabled"), kind: "ok" });
                        } catch (e) {
                          setToast({ text: String(e), kind: "err" });
                        }
                      }}
                    >
                      {t("Enable it")}
                    </button>
                  </div>
                )}
              </div>
            </Field>
          </>
        )}

        {section === "shortcuts" && (
          <>
            <div className="settings-title">{t("Shortcuts")}</div>
            <div className="settings-subtitle">{t("Global shortcuts")}</div>
            {hotkeyRow(
              "g-show",
              cfg.showHotkey,
              t("Open Clipboard History"),
              t("Global shortcut, works in any app")
            )}
            {hotkeyRow(
              "g-paste",
              cfg.pasteHotkey,
              t("Alternative Shortcut"),
              t("Second global shortcut for the same action")
            )}
            <div className="settings-hint">
              {t(
                "↑ ↓ move the selection, Esc closes. Hold Shift while pasting to strip formatting."
              )}
            </div>
            <div className="settings-subtitle">{t("Inside the window")}</div>
            {inApp.map(([field, title, desc]) => (
              <Field key={field} title={t(title)} desc={t(desc)}>
                <button
                  type="button"
                  className="hotkey-input"
                  data-recording={recording === `inapp:${field}`}
                  onClick={() => setRecording(recording === `inapp:${field}` ? null : `inapp:${field}`)}
                >
                  {recording === `inapp:${field}`
                    ? "Press keys…"
                    : pretty(cfg.inAppKeys[field]).join(" + ")}
                </button>
              </Field>
            ))}
            <div className="settings-hint">
              {t(
                "Click a shortcut and press the new combination. At least one modifier key (Ctrl, Alt, Shift or Win) is required."
              )}
            </div>
          </>
        )}

        {section === "paste" && (
          <>
            <div className="settings-title">{t("Pasting")}</div>
            <Field title={t("Paste as plain text")} desc={t("Strip formatting from every paste")}>
              <Toggle
                checked={cfg.pastePlainDefault}
                onChange={(v) => void update({ pastePlainDefault: v })}
              />
            </Field>
            <Field
              title={t("Keep window open after pasting")}
              desc={t("Paste several entries in a row")}
            >
              <Toggle
                checked={cfg.keepOpenAfterPaste}
                onChange={(v) => void update({ keepOpenAfterPaste: v })}
              />
            </Field>
          </>
        )}

        {section === "history" && (
          <>
            <div className="settings-title">{t("History")}</div>
            <Field title={t("Maximum entries")} desc={t("Oldest unpinned entries are removed first")}>
              <Dropdown
                value={cfg.maxEntries}
                onChange={(v) => void update({ maxEntries: v })}
                options={entryOptions}
              />
            </Field>
            <Field title={t("Keep history for")} desc={t("Pinned entries are never removed")}>
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
            <div className="settings-title">{t("Privacy & Indexing")}</div>
            <Field
              title={t("Ignore password managers")}
              desc={t("Never record content copied from 1Password, Bitwarden, KeePass…")}
            >
              <Toggle
                checked={cfg.ignorePasswordManagers}
                onChange={(v) => void update({ ignorePasswordManagers: v })}
              />
            </Field>
            <Field
              title={t("Also capture passwords & protected content")}
              desc={t(
                "Keeps what apps ask us not to record: password-manager copies, private/incognito browser windows and clipboard items marked as secret. Off by default."
              )}
            >
              <Toggle
                checked={cfg.captureSensitive}
                onChange={(v) => void update({ captureSensitive: v })}
              />
            </Field>
            <Field
              title={t("Index text in images (OCR)")}
              desc={t("Search inside screenshots with Windows OCR")}
            >
              <Toggle
                checked={cfg.ocrEnabled}
                onChange={(v) => void update({ ocrEnabled: v })}
              />
            </Field>
            <div className="settings-hint">
              {t(
                "Everything stays on this computer. To switch off every network request, see Preferences → Offline & private."
              )}
            </div>
          </>
        )}

        {section === "data" && (
          <>
            <div className="settings-title">{t("Data & Security")}</div>
            <Field
              title={t("Database encryption")}
              desc={t(
                "Clipboard text, rich text and images are stored in a SQLCipher database (AES-256 + HMAC-SHA512)"
              )}
            >
              <span className="db-badge" data-on={sys?.encrypted ? "yes" : "no"}>
                {sys?.encrypted ? t("Encrypted") : t("Unavailable")}
              </span>
            </Field>
            {sys?.keyError && (
              <div className="settings-hint" data-kind="warn">
                {t(
                  "The encrypted database could not be opened, so nothing is being saved this session: {msg}. The application fails closed on purpose — it never writes a key that is not protected by Windows, and never falls back to a predictable one.",
                  { msg: String(sys.keyError) }
                )}
              </div>
            )}
            <div className="settings-hint">
              {t(
                "The 256-bit key lives in rayclip.key, sealed with Windows DPAPI, so only your Windows user can open it — copying the files to another account or computer is useless. Everything (text, HTML/RTF, images) is inside rayclip.db; no loose files are left behind."
              )}
              {sys?.dbPath && (
                <>
                  <br />
                  <span className="db-path">{sys.dbPath}</span>
                </>
              )}
            </div>
            <Field
              title={t("Import from Raycast")}
              desc={t("Copies your Raycast clipboard history into this encrypted database")}
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
                      onSaved(t("Imported {n} items from Raycast", { n: total }));
                      void api.raycastImportCount().then(setRaycastCount).catch(() => {});
                    })
                    .catch((e) => {
                      setImporting(null);
                      onSaved(t("Import failed — {msg}", { msg: String(e) }), "err");
                    });
                }}
              >
                {importing
                  ? t("Importing {done}/{total}", {
                      done: importing.done,
                      total: importing.total || "…",
                    })
                  : raycastCount === null
                    ? t("Check Raycast history")
                    : raycastCount === 0
                      ? t("Raycast history not found")
                      : t("Import {n} items", { n: raycastCount })}
              </button>
            </Field>
            <div className="settings-hint">
              {t(
                "Raycast keeps its own database locked with a DPAPI key, but it also writes each item as a file in %LOCALAPPDATA%\Raycast\clipboard. The import reads those files (images, CF_HTML, RTF, text), keeps the original dates, and leaves the Raycast files untouched. Raycast itself is not installed or modified."
              )}
            </div>
          </>
        )}

        {section === "system" && (
          <>
            <div className="settings-title">{t("System")}</div>
            <Field
              title={t("Take over Win+V")}
              desc={t("Disable the native Windows clipboard panel so Win+V opens Rayclip")}
            >
              <Toggle
                checked={cfg.replaceWinV}
                onChange={(v) => void update({ replaceWinV: v })}
              />
            </Field>
            <div className="settings-hint">
              {t(
                'Windows keeps its own shortcut in Explorer. Rayclip writes DisabledHotkeys = "V" under HKCU\…\Explorer\Advanced (the same trick CopyQ and Ditto use). The change only applies after Explorer restarts.'
              )}
              <br />
              {t("Windows status:")}{" "}
              <strong>
                {sys?.winVDisabled ? t("Win+V disabled") : t("Win+V active in Windows")}
              </strong>
            </div>
            <Field
              title={t("Apply now")}
              desc={t("Restarts Explorer (open File Explorer windows close)")}
            >
              <button
                type="button"
                className="btn"
                onClick={() => {
                  void api
                    .restartExplorer()
                    .then(() => onSaved(t("Explorer restarted")))
                    .catch((e) =>
                      onSaved(t("Restart failed — {msg}", { msg: String(e) }), "err")
                    );
                }}
              >
                {t("Restart Explorer")}
              </button>
            </Field>
            <Field
              title={t("Start with Windows")}
              desc={t(
                "Registers a Task Scheduler job that starts Rayclip as administrator without a UAC prompt"
              )}
            >
              <Toggle
                checked={cfg.launchAtLogin}
                onChange={(v) => void update({ launchAtLogin: v })}
              />
            </Field>
            <div className="settings-hint">
              {t("Startup job:")}{" "}
              <strong>
                {sys?.startupTask ? t("scheduled task active") : t("not registered")}
              </strong>
              <br />
              {t("This window:")}{" "}
              <strong>
                {sys?.elevated ? t("running as administrator") : t("standard user")}
              </strong>
              <br />
              {t(
                "Rayclip is designed to run elevated: that is what lets it paste into elevated apps (Task Manager, admin consoles) and change Win+V and auto-start without being blocked. A UAC prompt appears on manual starts; the auto-start job and the relaunch task open it already elevated."
              )}
            </div>
          </>
        )}

        {section === "updates" && (
          <>
            <div className="settings-title">{t("Updates")}</div>
            <Field
              title={t("Installed version")}
              desc={t("Rayclip updates itself from the GitHub releases of its repository")}
            >
              <span className="about-badge">{`v${sys?.version ?? "—"}`}</span>
            </Field>
            <Field
              title={t("Check for updates automatically")}
              desc={t("Looks for a new release on start-up and every 6 hours. Never installs on its own.")}
            >
              <Toggle
                checked={cfg.autoUpdateCheck}
                onChange={(v) => void update({ autoUpdateCheck: v })}
              />
            </Field>
            <Field
              title={t("Check now")}
              desc={t("Ask GitHub for the latest release")}
            >
              <button
                type="button"
                className="btn"
                disabled={checking || installing}
                onClick={() => void checkNow()}
              >
                {checking ? t("Checking…") : t("Check for updates")}
              </button>
            </Field>
            {checkMsg && (
              <div className="settings-hint" data-kind={checkMsg.kind}>
                <span className={`status-dot ${checkMsg.kind}`} /> {checkMsg.text}
              </div>
            )}
            {release && !installing && !checking && (
              <div className="update-card">
                <div className="update-card-head">
                  <span className="update-badge">{`v${release.version}`}</span>
                  <span className="update-title">
                    {release.name || release.tag || `v${release.version}`}
                  </span>
                </div>
                {release.notes && (
                  <pre className="update-notes">{release.notes.slice(0, 1200)}</pre>
                )}
                <div className="update-actions">
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={!release.hasAsset}
                    title={
                      release.hasAsset
                        ? t("Download, check and restart in the new version")
                        : t("This release has no rayclip.exe to download")
                    }
                    onClick={() => void installNow()}
                  >
                    {t("Install update")}
                  </button>
                  {release.pageUrl && (
                    <button
                      type="button"
                      className="btn"
                      onClick={() => void api.openUrl(release.pageUrl)}
                    >
                      {t("Release page")}
                    </button>
                  )}
                </div>
              </div>
            )}
            {installing && (
              <div className="update-card">
                <div className="update-card-head">
                  <span className="update-badge">{`v${release?.version ?? ""}`}</span>
                  <span className="update-title">
                    {progress && progress.total > 0
                      ? t("Downloading… {pct}%", {
                          pct: Math.min(
                            100,
                            Math.round((progress.downloaded / progress.total) * 100)
                          ),
                        })
                      : t("Downloading…")}
                  </span>
                </div>
                <div className="update-bar">
                  <span
                    style={{
                      width: `${
                        progress && progress.total > 0
                          ? Math.min(100, (progress.downloaded / progress.total) * 100)
                          : 4
                      }%`,
                    }}
                  />
                </div>
              </div>
            )}
            <div className="settings-hint">
              {t(
                "The update downloads the new rayclip.exe, checks its SHA-256 against the one published in the release, and swaps the file in place. The running exe is only renamed, never overwritten, and the previous version is kept next to it until the next start — so a failed update always leaves you with a working Rayclip. Your history, settings and database are never touched."
              )}
            </div>
          </>
        )}

        {section === "about" && (
          <>
            <div className="settings-title">{t("About")}</div>
            <Field
              title="Rayclip"
              desc={t("Clipboard History for Windows, inspired by Raycast")}
            >
              <span className="about-badge">{sys?.version ? `v${sys.version}` : "—"}</span>
            </Field>
            <div className="settings-hint">
              <div>
                {t("Executable")}: <code>{sys?.exe || "—"}</code>
              </div>
              <div>
                {t("Data")}: <code>%APPDATA%\com.rayclip.app\clipboard</code>
              </div>
              <div className="about-line">
                <CheckIcon size={13} />{" "}
                {sys?.version ? `Rayclip ${sys.version} · ` : ""}
                {t("Built with Tauri + React · no account, no telemetry")}
              </div>
            </div>
          </>
        )}
      </div>

      {toast && (
        <div
          className="settings-toast"
          data-kind={toast.kind}
          role="status"

          onClick={() => setToast(null)}
        >
          <span className={`status-dot ${toast.kind}`} />
          {toast.text}
        </div>
      )}
      <button
        type="button"
        className="settings-close"
        onClick={onClose}
        title={t("Back (Esc)")}
      >
        Esc
      </button>
    </div>
  );
}
