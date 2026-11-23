import { useEffect, useRef } from "react";
import type { Entry } from "../lib/types";
import { displayTitle } from "../lib/format";
import { TrashIcon } from "../lib/icons";
import { kindLabel, type T } from "../lib/i18n";

/**
 * Confirmação para ações destrutivas em lote (apagar N entradas).
 *
 * Teclado: `Enter` confirma, `Esc` cancela, `←/→` troca o botão em foco.
 * Os eventos são tratados em capture com `stopImmediatePropagation` para não
 * vazarem para os atalhos do app (um `Esc` ali esconderia a janela).
 */
export function ConfirmDialog({
  title,
  message,
  items,
  confirmLabel,
  t,
  onCancel,
  onConfirm,
}: {
  title: string;
  message?: string;
  /** Até 5 entradas aparecem com o título; o resto vira "+N more". */
  items?: Entry[];
  confirmLabel: string;
  t?: T;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    // Cancelar vem focado: Enter acidental não apaga nada.
    cancelRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopImmediatePropagation();
        onCancel();
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        e.stopImmediatePropagation();
        // Enter confirma só quando o foco está no botão de apagar; com o foco
        // em Cancelar (padrão), Enter cancela.
        if (document.activeElement === confirmRef.current) onConfirm();
        else onCancel();
        return;
      }
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        e.stopImmediatePropagation();
        if (document.activeElement === confirmRef.current) cancelRef.current?.focus();
        else confirmRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onCancel, onConfirm]);

  const shown = items?.slice(0, 5) ?? [];
  const rest = (items?.length ?? 0) - shown.length;

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div className="confirm" role="alertdialog" aria-label={title}>
        <div className="confirm-head">
          <span className="confirm-glyph">
            <TrashIcon size={16} />
          </span>
          <div className="confirm-title">{title}</div>
        </div>
        {message && <div className="confirm-msg">{message}</div>}
        {shown.length > 0 && (
          <ul className="confirm-list">
            {shown.map((e) => (
              <li key={e.id}>
                <span className="confirm-kind">{kindLabel(e.kind, t ?? ((s: string) => s))}</span>
                <span className="confirm-item">{displayTitle(e, t)}</span>
              </li>
            ))}
            {rest > 0 && (
              <li className="confirm-more">
                {t ? t("+{n} more", { n: rest }) : `+${rest} more`}
              </li>
            )}
          </ul>
        )}
        <div className="confirm-foot">
          <span className="confirm-hint">
            {t ? t("Esc to cancel · ← → to switch") : "Esc to cancel · ← → to switch"}
          </span>
          <button type="button" className="btn" ref={cancelRef} onClick={onCancel}>
            {t ? t("Cancel") : "Cancel"}
          </button>
          <button
            type="button"
            className="btn btn-danger"
            ref={confirmRef}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
