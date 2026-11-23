import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { NavBar } from "./components/NavBar";
import { EntryList } from "./components/EntryList";
import { DetailsPane } from "./components/DetailsPane";
import { ActionBar } from "./components/ActionBar";
import { SettingsPage } from "./components/SettingsPage";
import { ActionPanel, type Action } from "./components/ActionPanel";
import { SaveManyDialog } from "./components/SaveManyDialog";
import * as api from "./api";
import { groupEntries, matchesFilter, matchesQuery, matchedOnlyInOcr } from "./lib/format";
import { DEFAULT_IN_APP_KEYS } from "./lib/types";
import { keycaps, matchAccel } from "./lib/keys";
import { applyTheme, setOsTransparency } from "./lib/theme";
import {
  BulletsIcon,
  CopyIcon,
  ExternalIcon,
  FileIcon,
  FolderIcon,
  PasteIcon,
  PinIcon,
  ClearIcon,
  TrashIcon,
  TextIcon,
  OcrIcon,
} from "./lib/icons";
import type { Entry, FilterId, Status } from "./lib/types";

export default function App() {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterId>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [panelIndex, setPanelIndex] = useState(0);
  const [hint, setHint] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [saveManyOpen, setSaveManyOpen] = useState(false);
  const [selection, setSelection] = useState<string[]>([]);
  const [fmtOpen, setFmtOpen] = useState(false);
  /** Id da entrada com o título em edição inline (Ctrl+E). */
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [cfg, setCfg] = useState<import("./lib/types").Settings | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async (keepSelection = true) => {
    const next = await api.listEntries();
    setEntries(next);
    setSelectedId((cur) => (keepSelection && cur && next.some((e) => e.id === cur) ? cur : next[0]?.id ?? null));
  }, []);

  // Uma cópia dispara 2–3 eventos (novo item, OCR, link). Recarrega uma vez só.
  const pending = useRef<number | null>(null);
  const refreshSoon = useCallback(() => {
    if (pending.current !== null) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(() => {
      pending.current = null;
      void refresh();
    }, 140);
  }, [refresh]);

  useEffect(() => {
    void refresh(false);
    // Indexa com OCR imagens antigas que ainda não têm texto.
    void api.ocrPending();
    void api.linkPending();
    void api.getSettings().then(setCfg);
    const subs = [
      listen("clipboard:pasted", () => report("ok", "Pasted into the app you came from")),
      listen("clipboard:new", refreshSoon),
      listen("clipboard:changed", refreshSoon),
      listen("clipboard:focus", () => {
        setQuery("");
        setPanelOpen(false);
        // Mantém a seleção (e a rolagem) de onde o usuário parou; só cai para o
        // item mais recente se a entrada selecionada não existir mais.
        void refresh(true).then(() => inputRef.current?.focus());
      }),
      // Ícone da bandeja → "Settings…"
      listen("rayclip:settings", () => setSettingsOpen(true)),
      // Configurações mudaram (tema, cor, preview no hover…)
      listen("settings:changed", () => void api.getSettings().then(setCfg)),
    ];
    inputRef.current?.focus();
    return () => {
      subs.forEach((p) => p.then((un) => un()));
    };
  }, [refresh]);

  // Tema e cor de destaque aplicados ao documento inteiro. O fundo translúcido
  // só vale se o Windows tiver os efeitos de transparência ligados.
  useEffect(() => {
    if (!cfg) return;
    let alive = true;
    applyTheme(cfg);
    void api.systemStatus().then((s) => {
      if (!alive) return;
      setOsTransparency(s.winTransparency !== false);
      applyTheme(cfg);
    });
    return () => {
      alive = false;
    };
  }, [cfg]);

  const visible = useMemo(
    () =>
      entries.filter(
        (e) => matchesFilter(e, filter) && matchesQuery(e, query)
      ),
    [entries, filter, query]
  );
  const buckets = useMemo(() => groupEntries(visible), [visible]);
  const flat = useMemo(() => buckets.flatMap((b) => b.entries), [buckets]);
  const selected = useMemo(
    () => flat.find((e) => e.id === selectedId) ?? null,
    [flat, selectedId]
  );

  /** Entradas marcadas, na ordem em que foram marcadas (Ctrl/Shift+clique). */
  const selectedEntries = useMemo(
    () =>
      selection
        .map((id) => entries.find((e) => e.id === id))
        .filter((e): e is Entry => !!e),
    [selection, entries]
  );

  /** Botão direito numa entrada abre o painel de ações (como no Raycast). */
  const openActionsFor = useCallback((id: string) => {
    setSelectedId(id);
    setPanelIndex(0);
    setFmtOpen(false);
    setPanelOpen(true);
  }, []);

  useEffect(() => {
    if (flat.length === 0) return;
    if (!selectedId || !flat.some((e) => e.id === selectedId)) {
      setSelectedId(flat[0].id);
    }
  }, [flat, selectedId]);

  // Miniatura real só do item selecionado (PDF, vídeo, doc) — nada em massa.
  useEffect(() => {
    const sel = entries.find((e) => e.id === selectedId);
    if (!sel || sel.hasThumb) return;
    if (!["file", "files", "pdf", "video", "audio"].includes(sel.kind)) return;
    void api.thumbEntry(sel.id).catch(() => {});
  }, [selectedId, entries]);

  useEffect(() => {
    if (!selectedId) return;
    listRef.current
      ?.querySelector<HTMLElement>(`[data-id="${selectedId}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selectedId]);

  const [status, setStatus] = useState<Status | null>(null);
  const statusTimer = useRef<number | null>(null);

  const report = useCallback((kind: "ok" | "err", text: string) => {
    setStatus({ kind, text });
    if (statusTimer.current) window.clearTimeout(statusTimer.current);
    statusTimer.current = window.setTimeout(() => setStatus(null), kind === "err" ? 6000 : 2600);
  }, []);

  const flash = useCallback(
    (text: string) => {
      setHint(text);
      report("ok", text);
      window.setTimeout(() => setHint(null), 1200);
    },
    [report]
  );

  /** Executa uma ação e transforma falhas em status vermelho com o motivo. */
  const attempt = useCallback(
    async (label: string, fn: () => Promise<unknown>, okText?: string) => {
      try {
        await fn();
        report("ok", okText ?? label + " · ok");
        return true;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        report("err", label + " failed — " + msg);
        return false;
      }
    },
    [report]
  );

  // Arrastar a janela: qualquer ponto da barra superior/rodapé. Um clique curto
  // continua indo para o campo de busca; só vira arrasto quando o mouse anda.
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const el = e.target as HTMLElement | null;
      if (!el) return;
      // Campos de texto e conteúdo selecionável nunca arrastam a janela:
      // senão não dá para selecionar/copiar/deletar texto com o mouse.
      if (el.closest("input, textarea, select, [contenteditable=true], button, .ab-group, [data-no-drag]")) return;
      if (!el.closest(".drag-strip, .actionbar-left, .nav")) return;

      const startX = e.clientX;
      const startY = e.clientY;
      let done = false;

      const cleanup = () => {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", cleanup);
      };
      const onMove = (m: MouseEvent) => {
        if (done) return;
        if (Math.hypot(m.clientX - startX, m.clientY - startY) < 4) return;
        done = true;
        cleanup();
        void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
          getCurrentWindow().startDragging().catch(() => {});
        });
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", cleanup);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, []);

  const paste = useCallback(
    async (id: string | null | undefined, plain = false, keepOpen = false) => {
      if (!id) return;
      const asPlain = plain || (cfg?.pastePlainDefault ?? false);
      setPanelOpen(false);
      setSelection([]);
      const ok = await attempt(
        asPlain ? "Paste as plain text" : "Paste",
        () => api.pasteEntry(id, asPlain, keepOpen),
        keepOpen ? "Pasted — window kept open" : "Pasted into the app you came from"
      );
      if (ok && !keepOpen) setQuery("");
    },
    [attempt, cfg?.pastePlainDefault]
  );

  /** Cola várias entradas em ordem (Ctrl/Shift+clique marcam, Enter cola). */
  const pasteSelection = useCallback(
    async (plain = false, keepOpen = false) => {
      if (selection.length < 2) return false;
      const asPlain = plain || (cfg?.pastePlainDefault ?? false);
      setPanelOpen(false);
      setFmtOpen(false);
      const count = selection.length;
      setSelection([]);
      await attempt(
        `Paste ${count} items in order`,
        () => api.pasteMany(selection, asPlain, keepOpen),
        `${count} items pasted in order`
      );
      setQuery("");
      return true;
    },
    [attempt, cfg?.pastePlainDefault, selection]
  );

  const saveAs = useCallback(
    async (format: string) => {
      if (!selected?.id) {
        setFmtOpen(false);
        return;
      }
      const hasContent =
        !!selected.hasImage || !!selected.text || selected.paths.length > 0;
      if (!hasContent) {
        setFmtOpen(false);
        report("err", "This entry has nothing to save");
        return;
      }
      setFmtOpen(false);
      setPanelOpen(false);
      try {
        const path = await api.saveEntryAs(selected.id, format);
        if (path) {
          report("ok", `Saved to ${path.split("\\").pop()}`);
        } else {
          report("ok", "Save cancelled");
        }
      } catch (e) {
        report("err", `Save failed — ${e instanceof Error ? e.message : String(e)}`);
      }
    },
    [selected?.id, report]
  );

  const copy = useCallback(
    async (id: string | null | undefined) => {
      if (!id) return;
      await attempt("Copy", () => api.copyEntry(id), "Copied to clipboard");
      setPanelOpen(false);
    },
    [attempt, cfg?.pastePlainDefault]
  );

  const startRename = useCallback((id: string | null | undefined) => {
    if (!id) return;
    setPanelOpen(false);
    setSelection([]);
    setSelectedId(id);
    setRenamingId(id);
  }, []);

  // Se a linha em edição sair da tela (busca, rolagem, exclusão), sai do modo.
  useEffect(() => {
    if (!renamingId) return;
    const t = window.setTimeout(() => {
      if (!document.querySelector(".rename-input")) setRenamingId(null);
    }, 500);
    return () => window.clearTimeout(t);
  }, [renamingId, entries]);

  /// Grava o novo título. `cancel` (Esc) ou nome vazio apenas saem da edição.
  const commitRename = useCallback(
    async (id: string, title: string, cancel = false) => {
      setRenamingId(null);
      const clean = title.trim();
      if (cancel || !clean) return;
      try {
        setEntries(await api.renameEntry(id, clean));
        flash("Renamed");
      } catch {
        report("err", "Could not rename");
      }
      // devolve o foco à busca para as setas/atalhos voltarem a funcionar
      inputRef.current?.focus();
    },
    [flash, report]
  );

  const move = useCallback(
    (delta: number) => {
      if (flat.length === 0) return;
      const idx = flat.findIndex((e) => e.id === selectedId);
      const next = Math.min(flat.length - 1, Math.max(0, (idx < 0 ? 0 : idx) + delta));
      setSelectedId(flat[next].id);
      setSelection([]);
    },
    [flat, selectedId]
  );

  /** Clique normal seleciona; Ctrl alterna; Shift pega o intervalo. */
  const selectWithMods = useCallback(
    (id: string, mods: { ctrl: boolean; shift: boolean }) => {
      if (mods.shift && selectedId) {
        const a = flat.findIndex((e) => e.id === selectedId);
        const b = flat.findIndex((e) => e.id === id);
        if (a >= 0 && b >= 0) {
          const slice = flat.slice(Math.min(a, b), Math.max(a, b) + 1).map((e) => e.id);
          setSelection(slice);
          return;
        }
      }
      if (mods.ctrl) {
        setSelection((prev) =>
          prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
        );
        setSelectedId(id);
        return;
      }
      setSelectedId(id);
      setSelection([]);
    },
    [flat, selectedId]
  );

  const remove = useCallback(
    async (id: string | null | undefined) => {
      if (!id) return;
      const next = await api.deleteEntry(id);
      setEntries(next);
      setPanelOpen(false);
      flash("Entry deleted");
    },
    [flash]
  );

  const actions: Action[] = useMemo(() => {
    if (!selected) return [];
    const K = cfg?.inAppKeys ?? DEFAULT_IN_APP_KEYS;
    if (fmtOpen) {
      // Submenu de formatos, conforme o tipo da entrada. Independe do tipo para
      // o painel não "voltar" quando o mouse passa por uma linha.
      const isImg = !!selected.hasImage;
      const fmts: { id: string; label: string; fmt: string }[] = isImg
        ? [
            { id: "fmt-png", label: "Save as PNG (default)", fmt: "png" },
            { id: "fmt-jpg", label: "Save as JPEG", fmt: "jpg" },
            { id: "fmt-webp", label: "Save as WebP", fmt: "webp" },
            { id: "fmt-bmp", label: "Save as BMP", fmt: "bmp" },
            { id: "fmt-gif", label: "Save as GIF", fmt: "gif" },
          ]
        : [
            { id: "fmt-txt", label: "Save as TXT (default)", fmt: "txt" },
            { id: "fmt-md", label: "Save as Markdown (.md)", fmt: "md" },
            { id: "fmt-json", label: "Save as JSON", fmt: "json" },
            { id: "fmt-csv", label: "Save as CSV", fmt: "csv" },
            { id: "fmt-html", label: "Save as HTML", fmt: "html" },
            { id: "fmt-rtf", label: "Save as Rich Text (.rtf)", fmt: "rtf" },
            { id: "fmt-url", label: "Save as Internet Shortcut (.url)", fmt: "url" },
            { id: "fmt-lua", label: "Save as Lua", fmt: "lua" },
            { id: "fmt-js", label: "Save as JavaScript", fmt: "js" },
            { id: "fmt-py", label: "Save as Python", fmt: "py" },
          ];
      return [
        ...fmts.map((f) => ({
          id: f.id,
          label: f.label,
          icon: <FileIcon size={15} />,
          run: () => void saveAs(f.fmt),
        })),
        {
          id: "fmt-back",
          label: "Back",
          icon: <BulletsIcon size={15} />,
          run: () => setFmtOpen(false),
        },
      ];
    }
    const list: Action[] = [
      ...(selection.length > 1
        ? [
            {
              id: "paste-many",
              label: `Paste ${selection.length} Items in Order`,
              icon: <PasteIcon size={15} />,
              keys: keycaps(K.paste),
              run: () => void pasteSelection(),
            },
            {
              id: "paste-many-keep",
              label: "Paste in Order and Keep Window Open",
              icon: <PasteIcon size={15} />,
              keys: keycaps(K.pasteKeepOpen),
              run: () => void pasteSelection(false, true),
            },
            {
              id: "save-many",
              label: `Save ${selection.length} Items As…`,
              icon: <FileIcon size={15} />,
              run: () => {
                setPanelOpen(false);
                setSaveManyOpen(true);
              },
            },
            {
              id: "clear-selection",
              label: `Clear Selection (${selection.length})`,
              icon: <BulletsIcon size={15} />,
              run: () => {
                setSelection([]);
                setPanelOpen(false);
              },
            },
          ]
        : []),
      {
        id: "paste",
        label: "Paste",
        icon: <PasteIcon size={15} />,
        keys: keycaps(K.paste),
        run: () => void paste(selected.id),
      },
      {
        id: "paste-plain",
        label: "Paste as Plain Text",
        icon: <TextIcon size={15} />,
        keys: [...keycaps(K.paste), "⇧"],
        run: () => void paste(selected.id, true),
      },
      {
        id: "paste-keep",
        label: "Paste and Keep Window Open",
        icon: <PasteIcon size={15} />,
        keys: keycaps(K.pasteKeepOpen),
        run: () => void paste(selected.id, false, true),
      },
      {
        id: "settings",
        label: "Settings…",
        icon: <BulletsIcon size={15} />,
        keys: keycaps(K.settings),
        run: () => {
          setPanelOpen(false);
          setSettingsOpen(true);
        },
      },
      {
        id: "copy",
        label: "Copy to Clipboard",
        icon: <CopyIcon size={15} />,
        keys: ["Ctrl", "C"],
        run: () => void copy(selected.id),
      },
      {
        id: "rename",
        label: "Rename Entry",
        icon: <FileIcon size={15} />,
        keys: keycaps(K.rename),
        run: () => startRename(selected.id),
      },
      {
        id: "pin",
        label: selected.pinned ? "Unpin Entry" : "Pin Entry",
        icon: <PinIcon size={15} />,
        keys: keycaps(K.pin),
        run: async () => {
          setEntries(await api.togglePin(selected.id));
          setPanelOpen(false);
        },
      },
    ];

    if (selected.kind === "link" && selected.text) {      list.push({
        id: "open-link",
        label: "Open Link",
        icon: <ExternalIcon size={15} />,
        keys: keycaps(K.openLink),
        run: () => {
          void api.openUrl(selected.text!);
          setPanelOpen(false);
        },
      });
    }
    if (selected.kind === "image") {
      if (selected.ocrText) {
        list.push({
          id: "copy-ocr",
          label: "Copy Text from Image",
          icon: <OcrIcon size={15} />,
          keys: keycaps(K.copyText),
          run: async () => {
            await api.copyOcrText(selected.id);
            flash("Image text copied");
            setPanelOpen(false);
          },
        });
      } else {
        list.push({
          id: "run-ocr",
          label: "Extract Text (OCR)",
          icon: <OcrIcon size={15} />,
          run: () => {
            void api.ocrEntry(selected.id);
            setPanelOpen(false);
            flash("Reading image…");
          },
        });
      }
    }
    if (selected.paths.length > 0) {
      list.push({
        id: "reveal",
        label: "Show in Folder",
        icon: <FolderIcon size={15} />,
        keys: ["Ctrl", "⇧", "F"],
        run: () => {
          void api.revealPath(selected.paths[0]);
          setPanelOpen(false);
        },
      });
    }

    list.push(
      {
        id: "delete",
        label: "Delete Entry",
        icon: <TrashIcon size={15} />,
        keys: keycaps(K.delete),
        danger: true,
        run: () => void remove(selected.id),
      },
      {
        id: "clear",
        label: "Clear All (keep pinned)",
        icon: <ClearIcon size={15} />,
        keys: keycaps(K.clearAll),
        danger: true,
        run: async () => {
          setEntries(await api.clearEntries(true));
          setPanelOpen(false);
          flash("History cleared");
        },
      }
    );
    const saveable =
      !!selected.hasImage || !!selected.text || selected.paths.length > 0;
    if (saveable) {
      list.splice(Math.max(0, list.findIndex((a) => a.id === "copy") + 1), 0, {
        id: "save-as",
        label: "Save As…",
        icon: <FileIcon size={15} />,
        run: () => {
          setFmtOpen(true);
          setPanelIndex(0); // o submenu de formatos começa no padrão
        },
      });
    }
    return list;
  }, [selected, paste, copy, remove, flash, cfg, selection, fmtOpen, pasteSelection, saveAs, startRename]);

  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (panelOpen) return; // panel handles its own keys
      if (saveManyOpen) return; // diálogo "Save N items" cuida do teclado
      if (document.body.dataset.recording) return; // recorder owns the keyboard
      if (document.body.dataset.menu) return; // dropdown owns the keyboard
      // Durante o renomear, quem cuida do teclado é o campo inline. Só
      // engolimos as teclas quando o foco está de fato nele (senão um campo
      // desmontado deixava a janela sem teclado nenhum).
      const inRename = !!(document.activeElement as HTMLElement | null)?.classList.contains(
        "rename-input"
      );
      if (renamingId && inRename) return;
      // Apagar texto na busca nunca é "apagar entrada": Backspace/Delete com o
      // cursor no campo pertencem ao campo.
      const el = e.target as HTMLElement | null;
      const typing =
        !!el &&
        (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
      if (typing && (e.key === "Backspace" || e.key === "Delete")) return;
      const K = cfg?.inAppKeys ?? DEFAULT_IN_APP_KEYS;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        move(1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        move(-1);
      } else if (matchAccel(e, K.settings)) {
        e.preventDefault();
        setSettingsOpen((v) => !v);
      } else if (matchAccel(e, K.actions)) {
        e.preventDefault();
        setPanelIndex(0);
        setPanelOpen(true);
      } else if (matchAccel(e, K.pasteKeepOpen)) {
        e.preventDefault();
        if (selection.length > 1) void pasteSelection(e.shiftKey, true);
        else void paste(selected?.id, e.shiftKey, true);
      } else if (matchAccel(e, K.paste)) {
        e.preventDefault();
        // Shift = texto puro
        if (selection.length > 1) void pasteSelection(e.shiftKey, false);
        else void paste(selected?.id, e.shiftKey, false);
      } else if (matchAccel(e, K.clearAll)) {
        e.preventDefault();
        setEntries(await api.clearEntries(true));
      } else if (matchAccel(e, K.delete)) {
        e.preventDefault();
        if (selected) setEntries(await api.deleteEntry(selected.id));
      } else if (matchAccel(e, K.rename) && selected) {
        e.preventDefault();
        startRename(selected.id);
      } else if (matchAccel(e, K.pin) && selected) {
        e.preventDefault();
        setEntries(await api.togglePin(selected.id));
      } else if (matchAccel(e, K.copyText) && selected?.ocrText) {
        e.preventDefault();
        void api.copyOcrText(selected.id).then(() => flash("Image text copied"));
      } else if (matchAccel(e, K.openLink) && selected?.kind === "link") {
        e.preventDefault();
        if (selected.text) void api.openUrl(selected.text);
      } else if (e.key === "Escape") {
        e.preventDefault();
        if (settingsOpen) {
          setSettingsOpen(false);
        } else if (selection.length > 0) {
          setSelection([]);
        } else if (query) {
          setQuery("");
        } else {
          void api.hideWindow();
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c" && selected) {
        const sel = window.getSelection()?.toString();
        if (!sel) {
          e.preventDefault();
          void copy(selected.id);
        }
      }
    };
        window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [move, paste, selected, query, panelOpen, copy, flash, settingsOpen, cfg, selection, pasteSelection, saveManyOpen, renamingId, startRename]);

  return (
    <div
      className="window"
      onContextMenu={(e) => {
        e.preventDefault();
        if (selected) openActionsFor(selected.id);
      }}
    >
      <div className="drag-strip" data-tauri-drag-region />
      {settingsOpen ? (
        <SettingsPage
          onClose={() => {
            setSettingsOpen(false);
            void api.getSettings().then(setCfg);
          }}
          onSaved={(msg, kind) => report(kind ?? "ok", msg)}
          onChange={setCfg}
        />
      ) : (
      <>
      <NavBar
        query={query}
        onQuery={setQuery}
        filter={filter}
        onFilter={setFilter}
        onBack={() => void api.hideWindow()}
        inputRef={inputRef}
      />
      <div className="body">
        <div className="list-pane" ref={listRef}>
          <EntryList
            buckets={buckets}
            selectedId={selected?.id ?? null}
            selection={selection}
            onSelect={selectWithMods}
            // "Preview on hover" desligado = só clique/↑↓ movem a seleção.
            onHover={cfg?.hoverPreview === false ? () => {} : setSelectedId}
            onActivate={(id) => void paste(id)}
            onPaste={(id) => void paste(id)}
            hasEntries={visible.length > 0}
            totalEntries={entries.length}
            query={query}
            isOcrHit={(e) => matchedOnlyInOcr(e, query)}
            onContextMenu={openActionsFor}
            renamingId={renamingId}
            onRename={(id, title, cancel) => void commitRename(id, title, cancel)}
          />
        </div>
        <DetailsPane entry={selected} selection={selection} entries={visible} />
      </div>
      <ActionBar
        onPaste={() => void paste(selected?.id)}
        onActions={() => {
          setPanelIndex(0);
          setPanelOpen(true);
        }}
        hint={hint}
        status={status}
        onSettings={() => setSettingsOpen(true)}
      />
      </>
      )}
      {panelOpen && !settingsOpen && (
        <ActionPanel
          actions={actions}
          index={Math.min(panelIndex, Math.max(0, actions.length - 1))}
          // Hover só move o destaque. (Antes isto fechava o submenu "Save Image
          // As…" ao menor movimento do mouse.)
          onIndex={setPanelIndex}
          onClose={() => {
            setFmtOpen(false);
            setPanelOpen(false);
          }}
        />
      )}
      {saveManyOpen && !settingsOpen && (
        <SaveManyDialog
          entries={selectedEntries}
          onClose={() => setSaveManyOpen(false)}
          onDone={(msg, kind) => report(kind ?? "ok", msg)}
        />
      )}
    </div>
  );
}
