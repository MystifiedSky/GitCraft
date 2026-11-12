import { useId, type ReactNode } from "react";

/** Keep focus out of closed sections while their height and opacity settle. */
export function Collapsible({ open, children, id }: { open: boolean; children: ReactNode; id?: string }) {
  const generated = useId();
  return (
    <div id={id ?? generated} className="section-collapse" data-open={open} aria-hidden={!open} inert={!open}>
      <div className="min-h-0 overflow-hidden">{children}</div>
    </div>
  );
}
