// shadcn-style wrappers around Radix dropdown and context menus.
import * as DM from "@radix-ui/react-dropdown-menu";
import * as CM from "@radix-ui/react-context-menu";
import type { ReactNode } from "react";

export interface MenuEntry {
  label?: string;
  shortcut?: string;
  icon?: ReactNode;
  disabled?: boolean;
  danger?: boolean;
  onSelect?: () => void;
  separator?: boolean;
  heading?: string;
  children?: MenuEntry[];
}

function DropItems({ items }: { items: MenuEntry[] }) {
  return (
    <>
      {items.map((it, i) => {
        if (it.separator) return <DM.Separator key={i} className="menu-sep" />;
        if (it.heading) return <DM.Label key={i} className="menu-label">{it.heading}</DM.Label>;
        if (it.children)
          return (
            <DM.Sub key={i}>
              <DM.SubTrigger className="menu-item" disabled={it.disabled}>
                {it.icon}
                {it.label}
                <span className="menu-shortcut">›</span>
              </DM.SubTrigger>
              <DM.Portal>
                <DM.SubContent className="menu-content" sideOffset={2}>
                  <DropItems items={it.children} />
                </DM.SubContent>
              </DM.Portal>
            </DM.Sub>
          );
        return (
          <DM.Item key={i} className="menu-item" disabled={it.disabled} onSelect={it.onSelect} style={it.danger ? { color: "#ef7a76" } : undefined}>
            {it.icon}
            {it.label}
            {it.shortcut && <span className="menu-shortcut">{it.shortcut}</span>}
          </DM.Item>
        );
      })}
    </>
  );
}

export function DropMenu({ trigger, items, align = "start" }: { trigger: ReactNode; items: MenuEntry[]; align?: "start" | "end" | "center" }) {
  return (
    <DM.Root modal={false}>
      <DM.Trigger asChild>{trigger}</DM.Trigger>
      <DM.Portal>
        <DM.Content className="menu-content" align={align} sideOffset={4}>
          <DropItems items={items} />
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  );
}

function CtxItems({ items }: { items: MenuEntry[] }) {
  return (
    <>
      {items.map((it, i) => {
        if (it.separator) return <CM.Separator key={i} className="menu-sep" />;
        if (it.heading) return <CM.Label key={i} className="menu-label">{it.heading}</CM.Label>;
        if (it.children)
          return (
            <CM.Sub key={i}>
              <CM.SubTrigger className="menu-item" disabled={it.disabled}>
                {it.icon}
                {it.label}
                <span className="menu-shortcut">›</span>
              </CM.SubTrigger>
              <CM.Portal>
                <CM.SubContent className="menu-content" sideOffset={2}>
                  <CtxItems items={it.children} />
                </CM.SubContent>
              </CM.Portal>
            </CM.Sub>
          );
        return (
          <CM.Item key={i} className="menu-item" disabled={it.disabled} onSelect={it.onSelect} style={it.danger ? { color: "#ef7a76" } : undefined}>
            {it.icon}
            {it.label}
            {it.shortcut && <span className="menu-shortcut">{it.shortcut}</span>}
          </CM.Item>
        );
      })}
    </>
  );
}

export function CtxMenu({ children, items, onOpenChange }: { children: ReactNode; items: MenuEntry[] | (() => MenuEntry[]); onOpenChange?: (open: boolean) => void }) {
  return (
    <CM.Root modal={false} onOpenChange={onOpenChange}>
      <CM.Trigger asChild>{children}</CM.Trigger>
      <CM.Portal>
        <CM.Content className="menu-content">
          <CtxItems items={typeof items === "function" ? items() : items} />
        </CM.Content>
      </CM.Portal>
    </CM.Root>
  );
}
