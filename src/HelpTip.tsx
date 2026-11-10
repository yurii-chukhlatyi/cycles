import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { CircleHelp } from "lucide-react";

export function HelpTip({ label, text }: { label: string; text: string }) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const hovered = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ left: number; top: number }>();
  const cancelClose = () => clearTimeout(timer.current);
  const show = () => {
    cancelClose();
    setOpen(true);
  };
  const enter = () => {
    hovered.current = true;
    show();
  };
  const leave = () => {
    hovered.current = false;
    cancelClose();
    timer.current = setTimeout(() => {
      if (!hovered.current && document.activeElement !== trigger.current)
        setOpen(false);
    }, 150);
  };
  useEffect(() => () => clearTimeout(timer.current), []);
  useLayoutEffect(() => {
    if (!open) return;
    const update = () => {
      if (!trigger.current || !tip.current) return;
      const rect = trigger.current.getBoundingClientRect();
      if (rect.bottom < 0 || rect.top > window.innerHeight) {
        setOpen(false);
        return;
      }
      const box = tip.current.getBoundingClientRect();
      setPosition({
        left: Math.max(
          16,
          Math.min(rect.left, window.innerWidth - box.width - 16),
        ),
        top:
          rect.bottom + 8 + box.height <= window.innerHeight - 16
            ? Math.max(16, rect.bottom + 8)
            : Math.max(16, rect.top - box.height - 8),
      });
    };
    const dismiss = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        setOpen(false);
      }
    };
    const outside = (event: PointerEvent) => {
      const target = event.target;
      if (
        target instanceof Node &&
        !trigger.current?.contains(target) &&
        !tip.current?.contains(target)
      )
        setOpen(false);
    };
    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    document.addEventListener("keydown", dismiss);
    document.addEventListener("pointerdown", outside);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
      document.removeEventListener("keydown", dismiss);
      document.removeEventListener("pointerdown", outside);
    };
  }, [open]);
  return (
    <>
      <button
        ref={trigger}
        type="button"
        className="help-button"
        aria-label={`Explain ${label}`}
        aria-describedby={open ? id : undefined}
        onMouseEnter={enter}
        onMouseLeave={leave}
        onFocus={show}
        onBlur={() => {
          if (!hovered.current) setOpen(false);
        }}
        onClick={show}
      >
        <CircleHelp size={14} aria-hidden="true" />
      </button>
      {open &&
        createPortal(
          <div
            ref={tip}
            id={id}
            role="tooltip"
            className="help-tooltip"
            style={{ ...position, visibility: position ? "visible" : "hidden" }}
            onMouseEnter={enter}
            onMouseLeave={leave}
          >
            {text}
          </div>,
          document.body,
        )}
    </>
  );
}

export function HelpLabel({ label, help }: { label: string; help: string }) {
  return (
    <span className="label-with-help">
      <span>{label}</span>
      <HelpTip label={label} text={help} />
    </span>
  );
}
