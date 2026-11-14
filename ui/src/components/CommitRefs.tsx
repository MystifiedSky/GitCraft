import { groupCommitRefs, type RefGroup } from "@/lib/commit-refs";
import { Check, ChevronDown, Cloud, Laptop, Tag } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { GitRef, RepoView } from "@/lib/api";
import { act } from "@/lib/repo";
import { branchMenu } from "@/lib/menus";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";
import { CtxMenu } from "./ui/menu";

interface Props {
  repo: RepoView;
  refs: GitRef[];
  color: string;
  hovered: boolean;
  selected: boolean;
  onSelect: (e: React.MouseEvent) => void;
  onHover: (active: boolean) => void;
}

/** Expand grouped refs into a colored label stack, outside the virtual scroller. */
export function CommitRefs({ repo, refs, color, hovered, selected, onSelect, onHover }: Props) {
  const m = useStore((s) => s.metrics);
  const id = useId();
  const anchor = useRef<HTMLButtonElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pointerInPopup = useRef(false);
  const contextOpen = useRef(false);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ left: 0, top: 0, maxHeight: 0, width: 0, above: false });
  const groups = useMemo(() => groupCommitRefs(refs), [refs]);
  const firstGroup = groups[0];
  const first = firstGroup.ref;
  const showPopup = open && groups.length > 1;
  const more = groups.length - 1;
  const expanded = groups.slice(0, m.ref_max);
  const menuFor = (group: RefGroup) => group.remote ? [
    ...branchMenu(repo, group.ref),
    { separator: true },
    { label: `Remote branch: ${group.remote.name}`, children: branchMenu(repo, group.remote) },
  ] : branchMenu(repo, group.ref);

  const cancelClose = () => clearTimeout(timer.current);
  const reveal = () => {
    cancelClose();
    setOpen(true);
  };
  const closeSoon = () => {
    cancelClose();
    timer.current = setTimeout(() => {
      if (contextOpen.current || pointerInPopup.current || popup.current?.contains(document.activeElement) || anchor.current === document.activeElement) return;
      setOpen(false);
    }, 120);
  };
  const menuChanged = (active: boolean) => {
    contextOpen.current = active;
    if (active) cancelClose();
    else { pointerInPopup.current = false; closeSoon(); }
  };
  useEffect(() => {
    if (hovered) reveal();
    else closeSoon();
    return cancelClose;
  }, [hovered]);
  useEffect(() => cancelClose, []);

  useLayoutEffect(() => {
    if (!showPopup || !anchor.current) return;
    const r = anchor.current.getBoundingClientRect();
    const below = window.innerHeight - r.top - 8;
    const above = r.bottom - 8;
    const desired = expanded.length * m.row_inner;
    const upwards = desired > below && above > below;
    const maxHeight = Math.min(desired, upwards ? above : below);
    const width = Math.min(320, window.innerWidth - 16);
    setPosition({
      left: Math.max(8, Math.min(r.left, window.innerWidth - width - 8)),
      top: upwards ? r.bottom - maxHeight : r.top,
      maxHeight,
      width,
      above: upwards,
    });
    const close = (e: Event) => {
      if (contextOpen.current) return;
      if (e.type === "scroll" && popup.current?.contains(e.target as Node)) return;
      cancelClose();
      setOpen(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || contextOpen.current) return;
      e.preventDefault();
      e.stopPropagation();
      // Restore keyboard focus before closing so focus doesn't immediately reopen it.
      anchor.current?.focus();
      setOpen(false);
    };
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", escape, true);
    return () => {
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", escape, true);
    };
  }, [showPopup, expanded.length, m.row_inner]);

  /** The hover tooltip: the name and what kind of label it is, e.g. "v0.1.3 (Tag)". */
  const describe = (r: GitRef) =>
    `${r.name} (${r.kind === "Tag" ? "Tag" : r.kind === "Remote" ? "Remote Branch" : r.name === repo.head_branch ? "Current Branch" : "Local Branch"})`;
  const icons = ({ ref: r, remote }: RefGroup, compact = false) => (
    <>
      {r.kind === "Local" && r.name === repo.head_branch && <Check size={13} strokeWidth={3} className="shrink-0" />}
      <span className={cn("min-w-0", compact ? "truncate font-semibold" : "break-all text-left")}>{r.name}</span>
      {r.kind === "Tag" && <Tag size={12} className="shrink-0" />}
      {r.kind === "Local" && <Laptop size={13} className="shrink-0 opacity-90" />}
      {(r.kind === "Remote" || remote) && <Cloud size={13} className="shrink-0 opacity-90" />}
      {r.kind === "Local" && (r.ahead > 0 || r.behind > 0) && (
        <span className="shrink-0 text-[10.5px] opacity-90" aria-label={`${r.ahead} ahead, ${r.behind} behind ${r.upstream}`}>
          {r.ahead > 0 && `↑${r.ahead}`}{r.behind > 0 && `↓${r.behind}`}
        </span>
      )}
    </>
  );
  const checkout = (r: GitRef) => {
    if (r.kind === "Local" && r.name !== repo.head_branch) void act(repo.path, { type: "checkout", target: r.name });
    if (r.kind === "Remote") void act(repo.path, { type: "checkoutRemote", remoteBranch: r.name });
  };
  const labelClass = "relative flex items-center gap-[4px] px-[5px] rounded-[2px] min-w-0 max-w-full text-[12px] text-fg outline-none focus-visible:ring-1 focus-visible:ring-accent";
  return (
    <>
      <CtxMenu items={() => menuFor(firstGroup)} onOpenChange={menuChanged}>
        <button
          ref={anchor}
          type="button"
          tabIndex={selected ? 0 : -1}
          aria-label={`${first.name}${firstGroup.remote ? ` tracking ${firstGroup.remote.name}` : ""}; ${groups.length} branch or tag labels`}
          title={describe(first) + (firstGroup.remote ? `; ${describe(firstGroup.remote)}` : "")}
          aria-expanded={groups.length > 1 ? showPopup : undefined}
          aria-controls={showPopup ? id : undefined}
          onFocus={reveal}
          onBlur={closeSoon}
          onMouseEnter={() => { reveal(); onHover(true); }}
          onMouseLeave={() => { closeSoon(); onHover(false); }}
          onClick={(e) => { e.stopPropagation(); onSelect(e); reveal(); }}
          onDoubleClick={(e) => { e.stopPropagation(); checkout(first); }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown" && groups.length > 1) {
              e.preventDefault();
              e.stopPropagation();
              reveal();
              requestAnimationFrame(() => popup.current?.querySelector<HTMLButtonElement>("button")?.focus());
            }
          }}
          className={cn(labelClass, "z-[1]")}
          style={{ background: `color-mix(in srgb, ${color} 28%, var(--color-app))`, height: m.row_inner, opacity: showPopup ? 0 : 1 }}
        >
          {icons(firstGroup, true)}
        </button>
      </CtxMenu>
      {more > 0 && (
        <>
          <span aria-hidden className="w-[5px] h-[2px] shrink-0" style={{ background: color, opacity: showPopup ? 0 : 1 }} />
          <button type="button" aria-label={`Show ${more} more branch or tag labels`} aria-expanded={showPopup} aria-controls={showPopup ? id : undefined}
            className="shrink-0 rounded-[2px] px-[4px] text-[11px] text-fg focus-visible:outline focus-visible:outline-accent"
            style={{ background: `color-mix(in srgb, ${color} 28%, var(--color-app))`, height: m.row_inner, opacity: showPopup ? 0 : 1 }}
            onFocus={reveal} onBlur={closeSoon} onMouseEnter={() => { reveal(); onHover(true); }} onMouseLeave={() => { closeSoon(); onHover(false); }}
            onClick={(e) => { e.stopPropagation(); reveal(); }}>+{more}</button>
        </>
      )}
      {showPopup && createPortal(
        <div
          id={id}
          ref={popup}
          role="group"
          aria-label="Branches and tags on this commit"
          className="commit-ref-stack fixed z-40 flex flex-col items-stretch overflow-y-auto rounded-[2px]"
          style={{ left: position.left, top: position.top, maxHeight: position.maxHeight, maxWidth: position.width, minWidth: Math.min(anchor.current?.offsetWidth ?? 0, position.width), background: `color-mix(in srgb, ${color} 28%, var(--color-app))`, transformOrigin: position.above ? "bottom left" : "top left" }}
          onMouseEnter={() => { pointerInPopup.current = true; cancelClose(); onHover(true); }}
          onMouseLeave={() => { pointerInPopup.current = false; closeSoon(); onHover(false); }}
          onFocus={cancelClose}
          onBlur={closeSoon}
          onKeyDown={(e) => {
            const buttons = [...(popup.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
            const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
            const next = e.key === "ArrowDown" ? (i + 1) % buttons.length : e.key === "ArrowUp" ? (i - 1 + buttons.length) % buttons.length : e.key === "Home" ? 0 : e.key === "End" ? buttons.length - 1 : -1;
            if (next >= 0) { e.preventDefault(); e.stopPropagation(); buttons[next]?.focus(); }
          }}
        >
          {expanded.map((group) => (
            <CtxMenu key={group.ref.full} items={() => menuFor(group)} onOpenChange={menuChanged}>
              <button
                type="button"
                title={describe(group.ref) + (group.remote ? `; ${describe(group.remote)}` : "")}
                className={cn(labelClass, "commit-ref-label shrink-0 w-full rounded-none hover:bg-white/10")}
                style={{ minHeight: m.row_inner }}
                onClick={(e) => { e.stopPropagation(); onSelect(e); }}
                onDoubleClick={(e) => { e.stopPropagation(); checkout(group.ref); }}
              >
                {icons(group, true)}
                {group === firstGroup && expanded.length > 1 && <ChevronDown size={11} className="shrink-0 opacity-60" />}
              </button>
            </CtxMenu>
          ))}
        </div>,
        document.body,
      )}
    </>
  );
}
