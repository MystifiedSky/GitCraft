import * as D from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { useRef, type ReactNode } from "react";
import { cn } from "@/lib/utils";

// shadcn-style dialog on Radix.
export function Dialog({
  open,
  onClose,
  title,
  children,
  footer,
  width = 440,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  const content = useRef<HTMLDivElement>(null);
  return (
    <D.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <D.Portal>
        <D.Overlay className="fixed inset-0 bg-black/50 z-50" />
        <D.Content
          ref={content}
          className="fixed left-1/2 top-[18%] -translate-x-1/2 z-50 bg-panel border border-white/12 rounded-md shadow-2xl flex flex-col max-h-[75vh]"
          style={{ width }}
          aria-describedby={undefined}
          onOpenAutoFocus={(event) => {
            const initial = content.current?.querySelector<HTMLElement>("[data-dialog-initial-focus]");
            if (initial) {
              event.preventDefault();
              initial.focus();
            }
          }}
        >
          <div className="flex items-center h-10 px-4 border-b border-line">
            <D.Title className="text-[14px] font-semibold text-white flex-1">{title}</D.Title>
            <D.Close aria-label="Close dialog" className="text-fg2 hover:text-white">
              <X size={16} />
            </D.Close>
          </div>
          <div className="p-4 overflow-auto">{children}</div>
          {footer && <div className="flex justify-end gap-2 px-4 pb-4">{footer}</div>}
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}

export function Field({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <label className={cn("flex flex-col gap-1 mb-3", className)}>
      <span className="text-[12px] text-fg2">{label}</span>
      {children}
    </label>
  );
}

export const inputClass =
  "h-8 px-2 rounded-[4px] bg-field border border-white/10 text-fg text-[13px] outline-none focus:border-accent placeholder:text-fg3";
