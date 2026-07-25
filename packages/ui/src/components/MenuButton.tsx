import { useEffect, useRef, useState, type ReactNode } from "react";

export interface MenuButtonProps {
  label: ReactNode;
  ariaLabel?: string;
  disabled?: boolean;
  children: (close: () => void) => ReactNode;
}

export function MenuButton({ label, ariaLabel, disabled, children }: MenuButtonProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handlePointerDown(e: PointerEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    }
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  return (
    <div className="menu-button" ref={rootRef}>
      <button
        type="button"
        className="btn"
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        onClick={() => setOpen((o) => !o)}
      >
        {label}
      </button>
      {open && (
        <div role="menu" className="menu-panel">
          {children(() => setOpen(false))}
        </div>
      )}
    </div>
  );
}
