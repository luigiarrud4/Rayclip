import { useEffect, useRef } from "react";
import type { JSX } from "react";

export interface Action {
  id: string;
  label: string;
  icon: JSX.Element;
  keys?: string[];
  danger?: boolean;
  run: () => void;
}

export function ActionPanel({
  actions,
  index,
  onIndex,
  onClose,
}: {
  actions: Action[];
  index: number;
  onIndex: (i: number) => void;
  onClose: () => void;
}) {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-idx="${index}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [index]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        onIndex((index + 1) % actions.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        onIndex((index - 1 + actions.length) % actions.length);
      } else if (e.key === "Enter") {
        e.preventDefault();
        actions[index]?.run();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [actions, index, onIndex, onClose]);

  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="panel">
        <div className="panel-header">
          <span className="title">Actions</span>
          <span>Esc to close</span>
        </div>
        <div className="panel-list" ref={listRef}>
          {actions.map((a, i) => (
            <div
              key={a.id}
              className="action"
              data-idx={i}
              data-selected={i === index}
              data-danger={a.danger}
              onMouseEnter={() => onIndex(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                a.run();
              }}
            >
              <span className="glyph">{a.icon}</span>
              <span className="label">{a.label}</span>
              {a.keys && (
                <span className="keys">
                  {a.keys.map((k) => (
                    <span className="keycap" key={k}>
                      {k}
                    </span>
                  ))}
                </span>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
