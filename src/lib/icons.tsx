import type { JSX } from "react";

type P = { size?: number; className?: string };

const svg = (children: JSX.Element, box = 16) =>
  function Icon({ size = 16, className }: P) {
    return (
      <svg
        width={size}
        height={size}
        viewBox={`0 0 ${box} ${box}`}
        fill="none"
        className={className}
        aria-hidden="true"
        style={{ display: "block", flexShrink: 0 }}
      >
        {children}
      </svg>
    );
  };

const S = {
  stroke: "currentColor",
  strokeWidth: 1.4,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export const ArrowLeft = svg(
  <path d="M9.8 3.5 5.3 8l4.5 4.5" {...S} />
);

export const ChevronDown = svg(
  <path d="m4.2 6.2 3.8 3.8 3.8-3.8" {...S} />
);

export const ChevronRight = svg(
  <path d="m6.2 4.2 3.8 3.8-3.8 3.8" {...S} />
);

export const Clipboard = svg(
  <>
    <rect x="3.4" y="3.2" width="9.2" height="10.4" rx="1.8" {...S} />
    <path d="M6 3.1V2.6A.6.6 0 0 1 6.6 2h2.8a.6.6 0 0 1 .6.6v.5" {...S} />
    <path d="M6 6.6h4M6 9h2.6" {...S} />
  </>
);

export const TextIcon = svg(
  <>
    <path d="M2.6 3.4h10.8M8 3.4v9.2M5.6 12.6h4.8" {...S} />
  </>
);

export const ImageIcon = svg(
  <>
    <rect x="2.2" y="3" width="11.6" height="10" rx="1.8" {...S} />
    <circle cx="5.9" cy="6.4" r="1.1" {...S} />
    <path d="m2.8 11.6 2.9-2.6 2.2 1.9 2.3-2.4 2.9 3.1" {...S} />
  </>
);

export const LinkIcon = svg(
  <>
    <path
      d="M6.4 9.6 9.6 6.4M7.3 4.9l1-1a2.5 2.5 0 0 1 3.6 3.6l-1 1M8.7 11.1l-1 1a2.5 2.5 0 0 1-3.6-3.6l1-1"
      {...S}
    />
  </>
);

export const AtIcon = svg(
  <>
    <circle cx="8" cy="8" r="2.4" {...S} />
    <path d="M10.4 5.6v3.2a2 2 0 0 0 4 0V8a6.4 6.4 0 1 0-2.5 5.1" {...S} />
  </>
);

export const DropperIcon = svg(
  <>
    <path d="m8.7 4.4 2.9 2.9M7.3 5.8l2.9 2.9M6 7.1l2.9 2.9-2.3 2.3H4.4v-2.2L6 7.1Z" {...S} />
    <path d="m10.2 2.9 1-1a1.9 1.9 0 0 1 2.7 2.7l-1 1" {...S} />
  </>
);

export const FileIcon = svg(
  <>
    <path d="M4 2.4h4.6L12 5.8v7.8H4z" {...S} />
    <path d="M8.4 2.6v3.3h3.3" {...S} />
  </>
);

export const FilesIcon = svg(
  <>
    <path d="M5.6 1.9h3.5l2.6 2.6v6.1H5.6z" {...S} />
    <path d="M3.1 4.1v9.9h6.7" {...S} />
  </>
);

export const PinIcon = svg(
  <>
    <path d="M9.9 2.6 13.4 6M11.6 4.3 8.9 7c-.5.5-1.4.6-2 .1L5 5.9l5.1 5.1-.9-1.9c-.5-.6-.4-1.5.1-2l2.7-2.7" {...S} />
    <path d="M6.6 9.4 3 13" {...S} />
  </>
);

export const CopyIcon = svg(
  <>
    <rect x="5.6" y="5.6" width="7.8" height="7.8" rx="1.6" {...S} />
    <path d="M10.4 5.4v-.9a1.6 1.6 0 0 0-1.6-1.6H4.2a1.6 1.6 0 0 0-1.6 1.6v4.6a1.6 1.6 0 0 0 1.6 1.6h.9" {...S} />
  </>
);

export const PasteIcon = svg(
  <>
    <path d="M6 2.9H4.3a1.4 1.4 0 0 0-1.4 1.4v8.3a1.4 1.4 0 0 0 1.4 1.4h7.4a1.4 1.4 0 0 0 1.4-1.4V4.3a1.4 1.4 0 0 0-1.4-1.4H10" {...S} />
    <rect x="6" y="1.9" width="4" height="2.4" rx="0.8" {...S} />
  </>
);

export const TrashIcon = svg(
  <>
    <path d="M2.9 4.4h10.2M6.3 4.2V2.9h3.4v1.3M4.5 4.6l.6 8.3h5.8l.6-8.3" {...S} />
  </>
);

export const ClearIcon = svg(
  <>
    <path d="M2.8 11.6h10.4M4.6 2.6h6.8v5.1a3.4 3.4 0 0 1-6.8 0z" {...S} />
  </>
);

export const ExternalIcon = svg(
  <>
    <path d="M8.8 2.9h4.3v4.3M13 3 7.7 8.3" {...S} />
    <path d="M11.3 9.7v2.9a.9.9 0 0 1-.9.9H3.6a.9.9 0 0 1-.9-.9V5.8a.9.9 0 0 1 .9-.9h2.8" {...S} />
  </>
);

export const FolderIcon = svg(
  <>
    <path d="M2.2 4.4a1.1 1.1 0 0 1 1.1-1.1h2.6l1.3 1.6h5.6a1.1 1.1 0 0 1 1.1 1.1v5.5a1.1 1.1 0 0 1-1.1 1.1H3.3a1.1 1.1 0 0 1-1.1-1.1z" {...S} />
  </>
);

export const CheckIcon = svg(
  <path d="m3.4 8.3 3 3 6.2-6.2" {...S} />
);

export const BulletsIcon = svg(
  <>
    <circle cx="3.6" cy="5" r="0.9" fill="currentColor" />
    <circle cx="3.6" cy="10.9" r="0.9" fill="currentColor" />
    <path d="M6.6 5h6.4M6.6 10.9h6.4" {...S} />
  </>
);

export const SearchIcon = svg(
  <>
    <circle cx="7.2" cy="7.2" r="4.1" {...S} />
    <path d="m10.4 10.4 3 3" {...S} />
  </>
);

export const OcrIcon = svg(
  <>
    <path d="M2.9 5.2V3.6a.7.7 0 0 1 .7-.7h1.6M13.1 5.2V3.6a.7.7 0 0 0-.7-.7h-1.6M2.9 10.8v1.6a.7.7 0 0 0 .7.7h1.6M13.1 10.8v1.6a.7.7 0 0 1-.7.7h-1.6" {...S} />
    <path d="M5.6 9.9 8 5.1l2.4 4.8M6.5 8.4h3" {...S} />
  </>
);

/** Expandir / prévia grande (Quick Look). */
export const ExpandIcon = svg(
  <>
    <path d="M9.4 2.7h3.9v3.9M13.3 2.7 8.6 7.4M6.6 13.3H2.7V9.4M2.7 13.3l4.7-4.7" {...S} />
  </>
);

export const ArrowUpIcon = svg(
  <path d="M8 12.6V3.4M4.4 7 8 3.4 11.6 7" {...S} />
);

export const ArrowDownIcon = svg(
  <path d="M8 3.4v9.2M4.4 9 8 12.6 11.6 9" {...S} />
);

/** Lápis: editar o conteúdo da entrada. */
export const PencilIcon = svg(
  <>
    <path d="M10.6 2.9 13.1 5.4 5.4 13.1 2.6 13.4l.3-2.8z" {...S} />
    <path d="m9.2 4.3 2.5 2.5" {...S} />
  </>
);

/** Relógio: "Delete Recent Entries…". */
export const ClockIcon = svg(
  <>
    <circle cx="8" cy="8" r="5.6" {...S} />
    <path d="M8 4.9V8l2.2 1.5" {...S} />
  </>
);

export const KindIcon = ({ kind }: { kind: string }) => {
  switch (kind) {
    case "image":
      return <ImageIcon />;
    case "link":
      return <LinkIcon />;
    case "email":
      return <AtIcon />;
    case "color":
      return <DropperIcon />;
    case "files":
      return <FilesIcon />;
    case "file":
    case "pdf":
    case "video":
    case "audio":
    case "archive":
      return <FileIcon />;
    default:
      return <TextIcon />;
  }
};
