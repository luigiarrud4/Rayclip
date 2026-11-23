import { useEffect, useRef, useState } from "react";
import type { JSX } from "react";
import {
  ArrowLeft,
  BulletsIcon,
  CheckIcon,
  ChevronDown,
  DropperIcon,
  ImageIcon,
  AtIcon,
  LinkIcon,
  TextIcon,
  FileIcon,
} from "../lib/icons";
import type { FilterId } from "../lib/types";

const OPTIONS: {
  group: "default" | "options";
  value: FilterId;
  label: string;
  icon: (props: { size?: number }) => JSX.Element;
}[] = [
  { group: "default", value: "all", label: "All Types", icon: BulletsIcon },
  { group: "options", value: "text", label: "Text Only", icon: TextIcon },
  { group: "options", value: "image", label: "Images Only", icon: ImageIcon },
  { group: "options", value: "file", label: "Files Only", icon: FileIcon },
  { group: "options", value: "link", label: "Links Only", icon: LinkIcon },
  { group: "options", value: "email", label: "Emails Only", icon: AtIcon },
  { group: "options", value: "color", label: "Colors Only", icon: DropperIcon },
];

export function NavBar({
  query,
  onQuery,
  filter,
  onFilter,
  onBack,
  inputRef,
}: {
  query: string;
  onQuery: (v: string) => void;
  filter: FilterId;
  onFilter: (f: FilterId) => void;
  onBack: () => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const wrapRef = useRef<HTMLDivElement>(null);
  const active = OPTIONS.find((o) => o.value === filter) ?? OPTIONS[0];

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setIndex((i) => (i + 1) % OPTIONS.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setIndex((i) => (i - 1 + OPTIONS.length) % OPTIONS.length);
      } else if (e.key === "Enter") {
        e.preventDefault();
        onFilter(OPTIONS[index].value);
        setOpen(false);
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, index, onFilter]);

  return (
    <header className="nav" data-tauri-drag-region>
      <button className="back-button" onClick={onBack} title="Close">
        <ArrowLeft size={18} />
      </button>
      <div className="search">
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          placeholder="Type to filter entries…"
          spellCheck={false}
          autoComplete="off"
        />
      </div>
      <div className="filter" ref={wrapRef}>
        <button
          className="filter-button"
          data-open={open}
          onClick={() => {
            setIndex(OPTIONS.findIndex((o) => o.value === filter));
            setOpen((o) => !o);
          }}
        >
          <span>{active.label}</span>
          <ChevronDown size={12} className="chevron" />
        </button>
        {open && (
          <div className="filter-menu">
            {OPTIONS.map((o, i) => (
              <div
                key={o.value}
                className="filter-item"
                data-active={i === index}
                data-selected={o.value === filter}
                onMouseEnter={() => setIndex(i)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  onFilter(o.value);
                  setOpen(false);
                  inputRef.current?.focus();
                }}
              >
                <span className="glyph">{o.icon({})}</span>
                <span className="label">{o.label}</span>
                <span className="check">
                  <CheckIcon size={13} />
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </header>
  );
}
