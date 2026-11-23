import { useEffect, useRef, useState } from "react";
import type { T } from "../lib/i18n";
import { TextIcon } from "../lib/icons";

/**
 * "Edit Content…" do Raycast: edita o **conteúdo** da entrada (não o título,
 * que é o `Ctrl+E`). O texto é carregado inteiro antes de abrir, porque a
 * lista só carrega os primeiros KB.
 *
 * Teclado: `Esc` cancela e `Ctrl+Enter` salva (Enter sozinho precisa existir
 * para quebrar linha dentro de um texto).
 */
export function EditContentDialog({
  title,
  initial,
  loading,
  t,
  onCancel,
  onSave,
}: {
  title: string;
  initial: string;
  loading?: boolean;
  t: T;
  onCancel: () => void;
  onSave: (text: string) => void;
}) {
  const [text, setText] = useState(initial);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  // Só sincroniza uma vez, quando o texto inteiro chega: sem isso o campo
  // guardaria a versão truncada da lista e salvar cortava a entrada.
  const synced = useRef(false);

  useEffect(() => {
    if (loading || synced.current) return;
    synced.current = true;
    setText(initial);
    areaRef.current?.focus();
  }, [loading, initial]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopImmediatePropagation();
        onCancel();
        return;
      }
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        e.stopImmediatePropagation();
        onSave(text);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onCancel, onSave, text]);

  const changed = text !== initial;

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div className="confirm editor" role="dialog" aria-label={t("Edit Content")}>
        <div className="confirm-head">
          <span className="confirm-glyph">
            <TextIcon size={16} />
          </span>
          <div className="confirm-title">{t("Edit Content")}</div>
        </div>
        <div className="confirm-msg">{title}</div>
        {loading ? (
          <div className="editor-loading">{t("Loading…")}</div>
        ) : (
          <textarea
            ref={areaRef}
            className="editor-area"
            value={text}
            spellCheck={false}
            placeholder={t("Add content…")}
            onChange={(e) => setText(e.target.value)}
          />
        )}
        <div className="confirm-foot">
          <span className="confirm-hint">
            {t("Esc to cancel · Ctrl+Enter to save")}
          </span>
          <button type="button" className="btn" onClick={onCancel}>
            {t("Cancel")}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={loading || !changed}
            onClick={() => onSave(text)}
          >
            {t("Save")}
          </button>
        </div>
      </div>
    </div>
  );
}
