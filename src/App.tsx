import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { NavBar } from "./components/NavBar";
import { EntryList, arrastandoAgora } from "./components/EntryList";
import { DetailsPane } from "./components/DetailsPane";
import { ActionBar } from "./components/ActionBar";
import { SettingsPage } from "./components/SettingsPage";
import { ActionPanel, type Action } from "./components/ActionPanel";
import { SaveManyDialog } from "./components/SaveManyDialog";
import { ImageViewer } from "./components/ImageViewer";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { EditContentDialog } from "./components/EditContentDialog";
import * as api from "./api";
import { groupEntries, matchedOnlyInOcr, mediaUrl } from "./lib/format";
import { DEFAULT_IN_APP_KEYS } from "./lib/types";
import { keycaps, matchAccel, matchAccelLoose } from "./lib/keys";
import { colorFormats } from "./lib/color";
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
  ExpandIcon,
  ArrowUpIcon,
  ArrowDownIcon,
  PencilIcon,
  ClockIcon,
  DropperIcon,
} from "./lib/icons";
import type { Entry, FilterId, Status, UpdateInfo } from "./lib/types";
import { makeT, type Lang } from "./lib/i18n";

/**
 * Entradas por página. A busca e o filtro rodam no Rust (8.44), então a página
 * recebe só isso em vez das 1,8 MB de todo o histórico.
 */
const PAGE = 200;

/** Janelas de tempo do "Delete Recent Entries…" (mesmas do Raycast). */
const RECENT_MINUTES = [5, 15, 30, 60, 1440];

function recentWindowLabel(min: number, t: (s: string, v?: Record<string, string | number>) => string) {
  if (min < 60) return t("Last {n} Minutes", { n: min });
  if (min === 60) return t("Last Hour");
  return t("Last 24 Hours");
}

/** "Últimos 5 minutos" para a mensagem de confirmação. */
function recentWhenLabel(min: number, t: (s: string, v?: Record<string, string | number>) => string) {
  if (min < 60) return t("{n} minutes", { n: min });
  if (min === 60) return t("1 hour");
  return t("24 hours");
}

export default function App() {
  const [entries, setEntries] = useState<Entry[]>([]);
  /** Quantos itens casam no total (vem do Rust); `entries` é só a página. */
  const [total, setTotal] = useState(0);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterId>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [panelIndex, setPanelIndex] = useState(0);
  const [hint, setHint] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // Seção das Configurações que abre ao clicar no aviso de atualização.
  const [settingsSection, setSettingsSection] = useState<"preferences" | "updates">(
    "preferences"
  );
  /** Release mais nova encontrada (aviso no rodapé + seção Updates). */
  const [release, setRelease] = useState<UpdateInfo | null>(null);
  const [saveManyOpen, setSaveManyOpen] = useState(false);
  // Visualizador grande da imagem selecionada (Espaço / clique na prévia).
  const [viewerEntry, setViewerEntry] = useState<Entry | null>(null);
  // "Armado" = o scroll sobre a prévia dá zoom (só depois de clicar na imagem).
  const [zoomArmed, setZoomArmed] = useState(false);
  /** Ids aguardando confirmação de exclusão (só para seleção múltipla). */
  const [confirmIds, setConfirmIds] = useState<string[] | null>(null);
  // Trocar de entrada (seta, clique, filtro) sempre desarma o zoom da prévia.
  useEffect(() => {
    setZoomArmed(false);
  }, [selectedId]);
  const [selection, setSelection] = useState<string[]>([]);
  const [fmtOpen, setFmtOpen] = useState(false);
  /** Submenus do painel de ações: "Copy Color as…" e "Delete Recent Entries…". */
  const [colorOpen, setColorOpen] = useState(false);
  const [recentOpen, setRecentOpen] = useState(false);
  /** Diálogo "Edit Content…" (conteúdo inteiro carregado antes de abrir). */
  const [editing, setEditing] = useState<{
    id: string;
    text: string;
    loading: boolean;
  } | null>(null);
  /** Janela (em minutos) aguardando confirmação de "apagar recentes". */
  const [confirmRecent, setConfirmRecent] = useState<number | null>(null);
  /** Id da entrada com o título em edição inline (Ctrl+E). */
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [cfg, setCfg] = useState<import("./lib/types").Settings | null>(null);
  // Idioma da interface: inglês por padrão, pt-BR quando o usuário escolhe.
  const lang: Lang = cfg?.language === "pt-BR" ? "pt-BR" : "en";
  const t = useMemo(() => makeT(lang), [lang]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Foco na busca com o texto selecionado: digitar substitui a busca anterior e
  // um Ctrl+A dentro do campo pega só o campo (não a página inteira).
  const focusSearch = useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    el.focus();
    el.select();
  }, []);


  // Parâmetros atuais da consulta num ref: `refresh` precisa ser estável (entra
  // em vários `useEffect`) mas tem de enxergar a busca do momento.
  const consulta = useRef({ query: "", filter: "all" });
  consulta.current = { query, filter: filter as string };

  // A seleção também vem de um ref, e não do fechamento: `refresh` precisa ser
  // **estável** para não recriar o efeito de montagem a cada clique (§8.47).
  const selecionadoRef = useRef<string | null>(null);
  selecionadoRef.current = selectedId;

  /**
   * Carrega a primeira página da busca atual.
   *
   * Mantém a seleção: se a entrada selecionada não estiver na primeira página
   * (histórico grande e algo antigo selecionado), pede as páginas seguintes até
   * achá-la — perder a seleção ao reabrir a janela estava fora de questão.
   */
  const refresh = useCallback(
    async (keepSelection = true) => {
      // Durante um arrasto para fora, re-renderizar troca a linha sob o cursor e
      // o Chromium cancela o arrasto. A lista volta a atualizar no próximo
      // refresh (o arrasto dura menos de um segundo).
      if (arrastandoAgora()) return;
      const { query: q, filter: f } = consulta.current;
      const sel = selecionadoRef.current;
      const page = await api.searchEntries(q, f, 0, PAGE);
      let items = page.items;
      if (keepSelection && sel && !items.some((e) => e.id === sel)) {
        for (let i = 1; i < 3 && items.length < page.total; i++) {
          const next = await api.searchEntries(q, f, i * PAGE, PAGE);
          if (next.items.length === 0) break;
          items = [...items, ...next.items];
          if (items.some((e) => e.id === sel)) break;
        }
      }
      setEntries(items);
      setTotal(page.total);
      setSelectedId((cur) =>
        keepSelection && cur && items.some((e) => e.id === cur) ? cur : items[0]?.id ?? null
      );
    },
    // De propósito vazio: a busca e a seleção entram por ref. Com `[selectedId]`
    // cada clique recriava esta função e o efeito de montagem (`[refresh, t]`)
    // rodava `refresh(false)` de novo, devolvendo a seleção ao primeiro item.
    []
  );

  /** Próxima página, pedida quando a rolagem chega perto do fim. */
  const carregando = useRef(false);
  const loadMore = useCallback(async () => {
    if (carregando.current || entries.length >= total) return;
    carregando.current = true;
    try {
      const { query: q, filter: f } = consulta.current;
      const page = await api.searchEntries(q, f, entries.length, PAGE);
      setEntries((cur) => {
        const tem = new Set(cur.map((e) => e.id));
        return [...cur, ...page.items.filter((e) => !tem.has(e.id))];
      });
      setTotal(page.total);
    } finally {
      carregando.current = false;
    }
  }, [entries.length, total]);

  // Uma cópia dispara 2–3 eventos (novo item, OCR, link). Recarrega uma vez só.
  const pending = useRef<number | null>(null);
  const refreshSoon = useCallback(() => {
    if (pending.current !== null) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(() => {
      pending.current = null;
      void refresh();
    }, 140);
  }, [refresh]);

  // Digitar/filtrar = nova consulta no Rust. 90 ms de debounce: tecla rápida
  // não vira 10 idas e voltas, e mesmo assim a lista acompanha o texto.
  const refreshAtual = useRef(refresh);
  refreshAtual.current = refresh;
  const primeiraConsulta = useRef(true);
  useEffect(() => {
    if (primeiraConsulta.current) {
      primeiraConsulta.current = false;
      return;
    }
    const id = window.setTimeout(() => {
      // `false`: com busca nova a seleção cai no primeiro resultado, que é o
      // comportamento do Raycast (e dá Enter para colar o mais provável).
      void refreshAtual.current(false);
    }, 90);
    return () => window.clearTimeout(id);
  }, [query, filter]);

  useEffect(() => {
    void refresh(false);
    // Indexa com OCR imagens antigas que ainda não têm texto.
    void api.ocrPending();
    void api.linkPending();
    void api.getSettings().then(setCfg);
    const subs = [
      listen("clipboard:pasted", () =>
        report("ok", t("Pasted into the app you came from"))
      ),
      listen("clipboard:new", refreshSoon),
      listen("clipboard:changed", refreshSoon),
      listen("clipboard:focus", () => {
        setQuery("");
        setPanelOpen(false);
        // Ao reabrir a janela o zoom da prévia volta a ficar solto: senão o
        // primeiro scroll já ampliaria em vez de rolar a coluna.
        setZoomArmed(false);
        // Mantém a seleção (e a rolagem) de onde o usuário parou; só cai para o
        // item mais recente se a entrada selecionada não existir mais.
        void refresh(true).then(focusSearch);
      }),
      // Ícone da bandeja → "Settings…"
      listen("rayclip:settings", () => setSettingsOpen(true)),
      // Atualização disponível: aviso no rodapé (nunca instala sozinho).
      listen<UpdateInfo>("update:available", (e) => setRelease(e.payload)),
      // Configurações mudaram (tema, cor, preview no hover…)
      listen("settings:changed", () => void api.getSettings().then(setCfg)),
    ];
    focusSearch();
    return () => {
      subs.forEach((p) => p.then((un) => un()));
    };
  }, [refresh, t]);

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

  // O Rust já devolveu filtrado e ordenado (§8.44): aqui só agrupamos por data
  // quando não há busca. Filtrar de novo daria resultado diferente — as regras
  // de ordenação são as mesmas, mas o conjunto carregado é só uma página.
  const visible = entries;
  // Com busca, um bloco único em ordem de relevância; sem busca, as datas do Raycast.
  const buckets = useMemo(
    () =>
      query.trim() ? [{ label: "", entries }] : groupEntries(entries, t),
    [entries, query, t]
  );
  const flat = useMemo(() => buckets.flatMap((b) => b.entries), [buckets]);
  const selected = useMemo(
    () => flat.find((e) => e.id === selectedId) ?? null,
    [flat, selectedId]
  );

  // O primeiro resultado sempre fica selecionado: sem isso, digitar até o item
  // anterior sair do filtro deixava a seleção nula e o Enter não colava nada.
  useEffect(() => {
    if (selection.length > 0) return;
    if (flat.length === 0) {
      if (selectedId !== null) setSelectedId(null);
      return;
    }
    if (selectedId && flat.some((e) => e.id === selectedId)) return;
    setSelectedId(flat[0].id);
  }, [flat, selectedId, selection.length]);

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

  // Batida para o vigia do renderizador (ver `heartbeat` no Rust).
  useEffect(() => {
    const id = window.setInterval(() => {
      void api.heartbeat().catch(() => {});
    }, 3000);
    void api.heartbeat().catch(() => {});
    return () => window.clearInterval(id);
  }, []);

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
        t(asPlain ? "Paste as plain text" : "Paste"),
        () => api.pasteEntry(id, asPlain, keepOpen),
        t(keepOpen ? "Pasted — window kept open" : "Pasted into the app you came from")
      );
      if (ok && !keepOpen) setQuery("");
    },
    [attempt, cfg?.pastePlainDefault, t]
  );

  /**
   * Cola várias entradas em ordem (Ctrl/Shift+clique marcam, Enter cola).
   *
   * Seleção **só de arquivos/imagens** (`paths` no disco ou imagem no banco) vai
   * por `pasteFiles`: uma `CF_HDROP` com todos os caminhos e **uma** colagem, que
   * é o que o Explorer entende como "cole estes N arquivos aqui". Qualquer outro
   * caso (texto, link, cor, misturado) continua colando em sequência.
   */
  const pasteSelection = useCallback(
    async (plain = false, keepOpen = false) => {
      if (selection.length < 2) return false;
      const asPlain = plain || (cfg?.pastePlainDefault ?? false);
      setPanelOpen(false);
      setFmtOpen(false);
      const count = selection.length;
      const escolhidas = selection
        .map((id) => entries.find((e) => e.id === id))
        .filter((e): e is Entry => !!e);
      const soArquivos =
        !asPlain &&
        escolhidas.length === count &&
        escolhidas.every((e) => e.paths.length > 0 || e.hasImage);
      setSelection([]);
      await attempt(
        t(soArquivos ? "Paste {n} files" : "Paste {n} items in order", { n: count }),
        () =>
          soArquivos
            ? api.pasteFiles(selection, keepOpen)
            : api.pasteMany(selection, asPlain, keepOpen),
        t(soArquivos ? "{n} files pasted" : "{n} items pasted in order", { n: count })
      );
      setQuery("");
      return true;
    },
    [attempt, cfg?.pastePlainDefault, entries, selection, t]
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
        report("err", t("This entry has nothing to save"));
        return;
      }
      setFmtOpen(false);
      setPanelOpen(false);
      try {
        const path = await api.saveEntryAs(selected.id, format);
        if (path) {
          report("ok", t("Saved to {name}", { name: path.split("\\").pop() ?? "" }));
        } else {
          report("ok", t("Save cancelled"));
        }
      } catch (e) {
        report(
          "err",
          t("Save failed — {msg}", { msg: e instanceof Error ? e.message : String(e) })
        );
      }
    },
    [selected?.id, report, t]
  );

  const copy = useCallback(
    async (id: string | null | undefined) => {
      if (!id) return;
      await attempt(t("Copy"), () => api.copyEntry(id), t("Copied to clipboard"));
      setPanelOpen(false);
    },
    [attempt, t]
  );

  const startRename = useCallback((id: string | null | undefined) => {
    if (!id) return;
    setPanelOpen(false);
    setSelection([]);
    setSelectedId(id);
    setRenamingId(id);
  }, []);

  /**
   * "Edit Content…": abre o editor já com o texto **inteiro**. A lista manda
   * só os primeiros KB, então o texto completo vem de `entry_text` antes de
   * mostrar o campo (sem isso, salvar truncava a entrada).
   */
  const startEdit = useCallback(async (e: Entry) => {
    if (!e.text) return;
    setPanelOpen(false);
    setSelection([]);
    setEditing({ id: e.id, text: e.text, loading: true });
    try {
      const full = await api.entryText(e.id);
      setEditing((cur) =>
        cur && cur.id === e.id ? { ...cur, text: full ?? cur.text, loading: false } : cur
      );
    } catch {
      setEditing((cur) => (cur ? { ...cur, loading: false } : cur));
    }
  }, []);

  const saveEdit = useCallback(
    async (text: string) => {
      const cur = editing;
      if (!cur) return;
      setEditing(null);
      try {
        await api.updateEntryContent(cur.id, text);
        await refresh();
        flash(t("Entry saved"));
      } catch (e) {
        report(
          "err",
          t("Could not save — {msg}", {
            msg: e instanceof Error ? e.message : String(e),
          })
        );
      }
      focusSearch();
    },
    [editing, flash, focusSearch, report, t]
  );

  /** "Copy Color as…": copia a cor no formato escolhido (texto puro). */
  const copyColor = useCallback(
    async (label: string, value: string) => {
      setColorOpen(false);
      setPanelOpen(false);
      try {
        await api.copyText(value);
        flash(t("Copied {label} to clipboard", { label }));
      } catch (e) {
        report(
          "err",
          t("Copy failed — {msg}", {
            msg: e instanceof Error ? e.message : String(e),
          })
        );
      }
    },
    [flash, report, t]
  );

  /** "Delete Recent Entries…": apaga só o que **não** está fixado. */
  const deleteRecent = useCallback(
    async (minutes: number) => {
      setConfirmRecent(null);
      try {
        const res = await api.deleteRecent(minutes);
        await refresh();
        // Sem a lista inteira de volta, a seleção múltipla é limpa: os ids que
        // saíram não podem continuar marcados.
        setSelection([]);
        flash(t("{n} recent entries deleted", { n: res.removed }));
      } catch (e) {
        report(
          "err",
          t("Delete failed - {msg}", {
            msg: e instanceof Error ? e.message : String(e),
          })
        );
      }
    },
    [entries.length, flash, report, t]
  );

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
        await api.renameEntry(id, clean);
        await refresh();
        flash(t("Renamed"));
      } catch {
        report("err", t("Could not rename"));
      }
      // devolve o foco à busca para as setas/atalhos voltarem a funcionar
      focusSearch();
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

  /** Apaga a seleção múltipla (o diálogo de confirmação chama isto). */
  const deleteMany = useCallback(
    async (ids: string[]) => {
      setConfirmIds(null);
      if (ids.length === 0) return;
      try {
        await api.deleteEntries(ids);
        await refresh();
        setSelection([]);
        setPanelOpen(false);
        flash(
          ids.length === 1
            ? t("Entry deleted")
            : t("{n} entries deleted", { n: ids.length })
        );
      } catch (e) {
        report(
          "err",
          t("Delete failed - {msg}", { msg: e instanceof Error ? e.message : String(e) })
        );
      }
    },
    [flash, report, t]
  );

  const remove = useCallback(
    async (id: string | null | undefined) => {
      if (!id) return;
      try {
        await api.deleteEntry(id);
        await refresh();
        setPanelOpen(false);
        setSelection((s) => s.filter((x) => x !== id));
        flash(t("Entry deleted"));
      } catch (e) {
        // Sem isto a promise rejeitava sem handler: nada aparecia e a entrada
        // seguia na lista, parecendo que a tecla Delete nao funcionou.
        report(
          "err",
          t("Delete failed - {msg}", { msg: e instanceof Error ? e.message : String(e) })
        );
      }
    },
    [flash, report, t]
  );

  const actions: Action[] = useMemo(() => {
    if (!selected) return [];
    const K = cfg?.inAppKeys ?? DEFAULT_IN_APP_KEYS;
    if (recentOpen) {
      // "Delete Recent Entries…" ≡ Raycast: janelas fixas de tempo, nunca
      // mexe nos fixados e sempre passa pela confirmação.
      return [
        ...RECENT_MINUTES.map((min) => ({
          id: `recent-${min}`,
          label: recentWindowLabel(min, t),
          icon: <ClockIcon size={15} />,
          danger: true,
          run: () => {
            setRecentOpen(false);
            setPanelOpen(false);
            setConfirmRecent(min);
          },
        })),
        {
          id: "recent-back",
          label: t("Back"),
          icon: <BulletsIcon size={15} />,
          run: () => setRecentOpen(false),
        },
      ];
    }
    if (colorOpen) {
      const fmts = colorFormats(selected.text ?? selected.title ?? "");
      return [
        ...fmts.map((f) => ({
          id: `color-${f.id}`,
          label: `${f.label} — ${f.value}`,
          icon: <CopyIcon size={15} />,
          run: () => void copyColor(f.label, f.value),
        })),
        {
          id: "color-back",
          label: t("Back"),
          icon: <BulletsIcon size={15} />,
          run: () => setColorOpen(false),
        },
      ];
    }
    if (fmtOpen) {
      // Submenu de formatos, conforme o tipo da entrada. Independe do tipo para
      // o painel não "voltar" quando o mouse passa por uma linha.
      const isImg = !!selected.hasImage;
      const fmts: { id: string; label: string; fmt: string }[] = isImg
        ? [
            { id: "fmt-png", label: t("Save as PNG (default)"), fmt: "png" },
            { id: "fmt-jpg", label: t("Save as JPEG"), fmt: "jpg" },
            { id: "fmt-webp", label: t("Save as WebP"), fmt: "webp" },
            { id: "fmt-bmp", label: t("Save as BMP"), fmt: "bmp" },
            { id: "fmt-gif", label: t("Save as GIF"), fmt: "gif" },
          ]
        : [
            { id: "fmt-txt", label: t("Save as TXT (default)"), fmt: "txt" },
            { id: "fmt-md", label: t("Save as Markdown (.md)"), fmt: "md" },
            { id: "fmt-json", label: t("Save as JSON"), fmt: "json" },
            { id: "fmt-csv", label: t("Save as CSV"), fmt: "csv" },
            { id: "fmt-html", label: t("Save as HTML"), fmt: "html" },
            { id: "fmt-rtf", label: t("Save as Rich Text (.rtf)"), fmt: "rtf" },
            { id: "fmt-url", label: t("Save as Internet Shortcut (.url)"), fmt: "url" },
            { id: "fmt-lua", label: t("Save as Lua"), fmt: "lua" },
            { id: "fmt-js", label: t("Save as JavaScript"), fmt: "js" },
            { id: "fmt-py", label: t("Save as Python"), fmt: "py" },
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
          label: t("Back"),
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
              label: t("Paste {n} Items in Order", { n: selection.length }),
              icon: <PasteIcon size={15} />,
              keys: keycaps(K.paste),
              run: () => void pasteSelection(),
            },
            {
              id: "paste-many-keep",
              label: t("Paste in Order and Keep Window Open"),
              icon: <PasteIcon size={15} />,
              keys: keycaps(K.pasteKeepOpen),
              run: () => void pasteSelection(false, true),
            },
            {
              id: "save-many",
              label: t("Save {n} Items As…", { n: selection.length }),
              icon: <FileIcon size={15} />,
              run: () => {
                setPanelOpen(false);
                setSaveManyOpen(true);
              },
            },
            {
              id: "delete-many",
              label: t("Delete {n} Entries", { n: selection.length }),
              icon: <TrashIcon size={15} />,
              keys: keycaps(K.delete),
              danger: true,
              run: () => {
                setPanelOpen(false);
                setConfirmIds([...selection]);
              },
            },
            {
              id: "clear-selection",
              label: t("Clear Selection ({n})", { n: selection.length }),
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
        label: t("Paste"),
        icon: <PasteIcon size={15} />,
        keys: keycaps(K.paste),
        run: () => void paste(selected.id),
      },
      {
        id: "paste-plain",
        label: t("Paste as Plain Text"),
        icon: <TextIcon size={15} />,
        keys: [...keycaps(K.paste), "⇧"],
        run: () => void paste(selected.id, true),
      },
      {
        id: "paste-keep",
        label: t("Paste and Keep Window Open"),
        icon: <PasteIcon size={15} />,
        keys: keycaps(K.pasteKeepOpen),
        run: () => void paste(selected.id, false, true),
      },
      {
        id: "settings",
        label: t("Settings…"),
        icon: <BulletsIcon size={15} />,
        keys: keycaps(K.settings),
        run: () => {
          setPanelOpen(false);
          setSettingsOpen(true);
        },
      },
      {
        id: "copy",
        label: t("Copy to Clipboard"),
        icon: <CopyIcon size={15} />,
        keys: ["Ctrl", "C"],
        run: () => void copy(selected.id),
      },
      {
        id: "rename",
        label: t("Rename Entry"),
        icon: <FileIcon size={15} />,
        keys: keycaps(K.rename),
        run: () => startRename(selected.id),
      },
      {
        id: "pin",
        label: t(selected.pinned ? "Unpin Entry" : "Pin Entry"),
        icon: <PinIcon size={15} />,
        keys: keycaps(K.pin),
        run: async () => {
          await api.togglePin(selected.id);
          await refresh();
          setPanelOpen(false);
        },
      },
    ];

    // Fixado: dá para reordenar dentro do grupo "Fixados" (igual ao Raycast).
    if (selected.pinned && selection.length <= 1) {
      list.push(
        {
          id: "pin-up",
          label: t("Move Up in Pinned"),
          icon: <ArrowUpIcon size={15} />,
          keys: ["Ctrl", "⇧", "↑"],
          run: async () => {
            await api.movePinned(selected.id, true);
            await refresh();
            setPanelOpen(false);
          },
        },
        {
          id: "pin-down",
          label: t("Move Down in Pinned"),
          icon: <ArrowDownIcon size={15} />,
          keys: ["Ctrl", "⇧", "↓"],
          run: async () => {
            await api.movePinned(selected.id, false);
            await refresh();
            setPanelOpen(false);
          },
        }
      );
    }

    // Editar o conteúdo (o título continua sendo o Ctrl+E).
    if (selected.text) {
      list.push({
        id: "edit-content",
        label: t("Edit Content…"),
        icon: <PencilIcon size={15} />,
        keys: ["Ctrl", "⇧", "E"],
        run: () => void startEdit(selected),
      });
    }

    // Cor: copiar em outros formatos (HEX, RGB, HSL, HSV…).
    if (selected.kind === "color") {
      list.push({
        id: "copy-color",
        label: t("Copy Color as…"),
        icon: <DropperIcon size={15} />,
        run: () => {
          setColorOpen(true);
          setPanelIndex(0);
        },
      });
    }

    if (selected.kind === "link" && selected.text) {      list.push({
        id: "open-link",
        label: t("Open Link"),
        icon: <ExternalIcon size={15} />,
        keys: keycaps(K.openLink),
        run: () => {
          void api.openUrl(selected.text!);
          setPanelOpen(false);
        },
      });
    }
    if (selected.kind === "image") {
      list.push({
        id: "open-preview",
        label: t("Open Preview"),
        icon: <ExpandIcon size={15} />,
        keys: ["Space"],
        run: () => {
          setZoomArmed(false);
          setViewerEntry(selected);
          setPanelOpen(false);
        },
      });
      if (selected.ocrText) {
        list.push({
          id: "copy-ocr",
          label: t("Copy Text from Image"),
          icon: <OcrIcon size={15} />,
          keys: keycaps(K.copyText),
          run: async () => {
            await api.copyOcrText(selected.id);
            flash(t("Image text copied"));
            setPanelOpen(false);
          },
        });
      } else {
        list.push({
          id: "run-ocr",
          label: t("Extract Text (OCR)"),
          icon: <OcrIcon size={15} />,
          run: () => {
            void api.ocrEntry(selected.id);
            setPanelOpen(false);
            flash(t("Reading image…"));
          },
        });
      }
    }
    if (selected.paths.length > 0) {
      list.push({
        id: "reveal",
        label: t("Show in Folder"),
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
        id: "delete-recent",
        label: t("Delete Recent Entries…"),
        icon: <ClockIcon size={15} />,
        danger: true,
        run: () => {
          setRecentOpen(true);
          setPanelIndex(0);
        },
      },
      {
        id: "delete",
        label: t("Delete Entry"),
        icon: <TrashIcon size={15} />,
        keys: keycaps(K.delete),
        danger: true,
        run: () => void remove(selected.id),
      },
      {
        id: "clear",
        label: t("Clear All (keep pinned)"),
        icon: <ClearIcon size={15} />,
        keys: keycaps(K.clearAll),
        danger: true,
        run: async () => {
          await api.clearEntries(true);
          await refresh();
          setPanelOpen(false);
          flash(t("History cleared"));
        },
      }
    );
    const saveable =
      !!selected.hasImage || !!selected.text || selected.paths.length > 0;
    if (saveable) {
      list.splice(Math.max(0, list.findIndex((a) => a.id === "copy") + 1), 0, {
        id: "save-as",
        label: t("Save As…"),
        icon: <FileIcon size={15} />,
        run: () => {
          setFmtOpen(true);
          setPanelIndex(0); // o submenu de formatos começa no padrão
        },
      });
    }
    return list;
  }, [selected, paste, copy, remove, flash, cfg, selection, fmtOpen, pasteSelection, saveAs, startRename, t, colorOpen, recentOpen, copyColor, startEdit]);

  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      try {
      if (panelOpen) return; // panel handles its own keys
      if (saveManyOpen) return; // diálogo "Save N items" cuida do teclado
      if (confirmIds) return; // confirmação de exclusão cuida do teclado
      if (confirmRecent !== null) return; // "apagar recentes" também
      if (editing) return; // editor de conteúdo cuida do teclado
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
      if (settingsOpen && !typing) {
        // Com Configuracoes abertas o teclado e dela. Antes so o Esc era
        // tratado aqui: Delete ainda apagava a entrada selecionada atras da
        // pagina e os atalhos de colar copiavam por baixo dela.
        if (e.key === "Escape") {
          e.preventDefault();
          setSettingsOpen(false);
        }
        return;
      }
      const K = cfg?.inAppKeys ?? DEFAULT_IN_APP_KEYS;

      // Reordenar fixados (Ctrl+Shift+↑/↓) vem antes das setas simples.
      if (
        selected?.pinned &&
        (e.ctrlKey || e.metaKey) &&
        e.shiftKey &&
        (e.key === "ArrowUp" || e.key === "ArrowDown")
      ) {
        e.preventDefault();
        await api.movePinned(selected.id, e.key === "ArrowUp");
        await refresh();
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        move(1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        move(-1);
      } else if (matchAccel(e, K.settings)) {
        e.preventDefault();
        setSettingsOpen((v) => !v);
      } else if (
        (e.key === " " || e.code === "Space") &&
        !query.trim() &&
        selected?.kind === "image" &&
        selected.hasImage
      ) {
        // Espaço só abre a prévia quando não há busca digitada — senão ele
        // continua sendo espaço no campo de filtro.
        e.preventDefault();
        setZoomArmed(false);
        // Solta o foco da busca **antes** de abrir: com o campo focado, as
        // teclas seguintes (inclusive Espaço) continuariam entrando no filtro
        // por cima da imagem.
        inputRef.current?.blur();
        setViewerEntry(selected);
      } else if (matchAccel(e, K.actions)) {
        e.preventDefault();
        setPanelIndex(0);
        setPanelOpen(true);
      } else if (matchAccelLoose(e, K.pasteKeepOpen)) {
        e.preventDefault();
        if (selection.length > 1) void pasteSelection(e.shiftKey, true);
        else void paste(selected?.id, e.shiftKey, true);
      } else if (matchAccelLoose(e, K.paste)) {
        e.preventDefault();
        // Shift = texto puro
        if (selection.length > 1) void pasteSelection(e.shiftKey, false);
        else void paste(selected?.id, e.shiftKey, false);
      } else if (matchAccel(e, K.clearAll)) {
        e.preventDefault();
        await api.clearEntries(true);
          await refresh();
      } else if (matchAccel(e, K.delete)) {
        e.preventDefault();
        // Seleção múltipla apaga todas as marcadas — e isso pede confirmação.
        if (selection.length > 1) setConfirmIds([...selection]);
        else if (selected) await remove(selected.id);
      } else if (matchAccel(e, K.rename) && selected) {
        e.preventDefault();
        startRename(selected.id);
      } else if (
        (e.ctrlKey || e.metaKey) &&
        e.shiftKey &&
        e.key.toLowerCase() === "e" &&
        selected?.text
      ) {
        // Editar o conteúdo (Ctrl+Shift+E) — Ctrl+E continua sendo o título.
        e.preventDefault();
        void startEdit(selected);
      } else if (matchAccel(e, K.pin) && selected) {
        e.preventDefault();
        await api.togglePin(selected.id);
        await refresh();
      } else if (matchAccel(e, K.copyText) && selected?.ocrText) {
        e.preventDefault();
        void api.copyOcrText(selected.id).then(() => flash(t("Image text copied")));
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
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c" && selected && !typing) {
        const sel = window.getSelection()?.toString();
        if (!sel) {
          e.preventDefault();
          void copy(selected.id);
        }
      }
      } catch (err) {
        // invoke rejeitado (banco ocupado, entrada apagada por outro caminho)
        // nao pode virar rejeicao solta no handler de teclado.
        report(
          "err",
          t("Key action failed - {msg}", {
            msg: err instanceof Error ? err.message : String(err),
          })
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [move, paste, selected, query, panelOpen, copy, flash, settingsOpen, cfg, selection, pasteSelection, saveManyOpen, renamingId, startRename, report, remove, t, editing, confirmRecent, startEdit]);

  return (
    <div
      className="window"
      onContextMenu={(e) => {
        e.preventDefault();
        if (selected) openActionsFor(selected.id);
      }}
    >
      {/* Sem `title`: o tooltip nativo do WebView2 ficava na tela depois que a
          janela escondia, e reaparecia toda vez que o mouse passava na borda
          (o `WM_NCHITTEST` da moldura continua valendo com a janela visível). */}
      <div className="drag-strip" data-tauri-drag-region />
      {settingsOpen ? (
        <SettingsPage
          onClose={() => {
            setSettingsOpen(false);
            setSettingsSection("preferences");
            void api.getSettings().then(setCfg);
          }}
          onSaved={(msg, kind) => report(kind ?? "ok", msg)}
          onChange={setCfg}
          t={t}
          startSection={settingsSection}
          availableUpdate={release}
          onUpdateFound={setRelease}
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
        locked={!!viewerEntry}
        t={t}
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
            totalEntries={total}
            query={query}
            isOcrHit={(e) => matchedOnlyInOcr(e, query)}
            onNearEnd={() => void loadMore()}
            onContextMenu={openActionsFor}
            renamingId={renamingId}
            onRename={(id, title, cancel) => void commitRename(id, title, cancel)}
            t={t}
          />
        </div>
        <DetailsPane
          entry={selected}
          selection={selection}
          entries={visible}
          zoomArmed={zoomArmed}
          onZoomArmedChange={setZoomArmed}
          onOpenImage={(e) => {
            setZoomArmed(false);
            setViewerEntry(e);
          }}
          t={t}
        />
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
        keys={cfg?.inAppKeys}
        updateAvailable={release?.version ?? null}
        onUpdate={() => {
          setSettingsSection("updates");
          setSettingsOpen(true);
        }}
        t={t}
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
            setColorOpen(false);
            setRecentOpen(false);
            setPanelOpen(false);
          }}
          t={t}
        />
      )}
      {saveManyOpen && !settingsOpen && (
        <SaveManyDialog
          entries={selectedEntries}
          onClose={() => setSaveManyOpen(false)}
          onDone={(msg, kind) => report(kind ?? "ok", msg)}
          t={t}
          lang={lang}
        />
      )}
      {viewerEntry && (
        <ImageViewer
          src={mediaUrl("img", viewerEntry.id, viewerEntry.createdAt)}
          title={viewerEntry.title || viewerEntry.text || t("Image")}
          gif={viewerEntry.gif}
          onClose={() => setViewerEntry(null)}
          t={t}
        />
      )}
      {confirmIds && (
        <ConfirmDialog
          title={t("Delete {n} entries?", { n: confirmIds.length })}
          message={t(
            "They are removed from the history for good — pinned entries included. This can't be undone."
          )}
          items={confirmIds
            .map((id) => entries.find((e) => e.id === id))
            .filter((e): e is Entry => !!e)}
          confirmLabel={t("Delete {n}", { n: confirmIds.length })}
          t={t}
          onCancel={() => setConfirmIds(null)}
          onConfirm={() => void deleteMany(confirmIds)}
        />
      )}
      {editing && (
        <EditContentDialog
          title={editing.text.split("\n")[0].slice(0, 80)}
          initial={editing.text}
          loading={editing.loading}
          t={t}
          onCancel={() => {
            setEditing(null);
            focusSearch();
          }}
          onSave={(text) => void saveEdit(text)}
        />
      )}
      {confirmRecent !== null && (
        <ConfirmDialog
          title={t("Delete recent entries?")}
          message={t(
            "Entries copied in the last {when} are removed for good. Pinned entries are kept.",
            { when: recentWhenLabel(confirmRecent, t) }
          )}
          items={entries.filter(
            (e) =>
              !e.pinned &&
              e.createdAt >=
                Date.now() - confirmRecent * 60_000
          )}
          confirmLabel={t("Delete {n}", {
            n: entries.filter(
              (e) => !e.pinned && e.createdAt >= Date.now() - confirmRecent * 60_000
            ).length,
          })}
          t={t}
          onCancel={() => setConfirmRecent(null)}
          onConfirm={() => void deleteRecent(confirmRecent)}
        />
      )}
    </div>
  );
}
