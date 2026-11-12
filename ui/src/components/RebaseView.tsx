import { ArrowDown, ArrowUp, GripVertical, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Step } from "@/lib/api";
import { useStore, type RebasePlan } from "@/lib/store";
import { runPlan } from "@/lib/rewrite";
import { Button } from "./ui/button";
import { Avatar } from "./Icons";
import { cn } from "@/lib/utils";

const STEPS: { step: Step; label: string; key: string; color: string }[] = [
  { step: "pick", label: "Pick", key: "P", color: "#5cb85c" },
  { step: "reword", label: "Reword", key: "R", color: "#4d88ff" },
  { step: "squash", label: "Squash", key: "S", color: "#de9b43" },
  { step: "drop", label: "Drop", key: "D", color: "#d9413d" },
];

/** Interactive Rebase / Interactive Cherry Pick, opened in the centre like the original.
 *  Newest commit at the top. Keys: P / R / S / D set the action, ↑ ↓ select, Ctrl+↑ ↓ move. */
export function RebaseView({ tabId, path, plan }: { tabId: string; path: string; plan: RebasePlan }) {
  const s = useStore();
  const n = s.metrics.sha_len;
  const [sel, setSel] = useState(0);
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const firstItem = useRef<HTMLButtonElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const message = useRef<HTMLTextAreaElement>(null);
  const cherry = plan.mode === "cherry";
  const items = plan.items;
  const update = (next: RebasePlan["items"]) => s.setUi(tabId, { rebase: { ...plan, items: next } });
  const select = (i: number) => {
    setSel(i);
    root.current?.querySelector<HTMLButtonElement>(`[data-rebase-item="${i}"]`)?.focus();
  };

  const setAction = (i: number, step: Step) => {
    if (!items[i] || (cherry && step === "squash" && i === items.length - 1)) return;
    update(items.map((it, k) => (k === i ? { ...it, action: step } : it)));
    if (step === "reword") {
      setEditing(i);
      setDraft(items[i].message);
    }
  };
  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= items.length) return;
    const out = [...items];
    [out[i], out[j]] = [out[j], out[i]];
    update(out);
    setSel(j);
  };

  useEffect(() => {
    (firstItem.current ?? closeButton.current)?.focus();
  }, []);

  useEffect(() => {
    if (editing !== null) message.current?.focus();
  }, [editing]);

  // These shortcuts apply throughout the editor, while focus stays on real controls.
  useEffect(() => {
    const panel = root.current;
    if (!panel) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing || (e.target as HTMLElement).closest("textarea")) return;
      const st = STEPS.find((x) => x.key.toLowerCase() === e.key.toLowerCase());
      if (st && !e.ctrlKey && !e.metaKey && !e.altKey) return (e.preventDefault(), setAction(sel, st.step));
      if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        e.preventDefault();
        const dir = e.key === "ArrowUp" ? -1 : 1;
        if (e.ctrlKey || e.metaKey || e.altKey) move(sel, dir);
        else if (items.length) select(Math.max(0, Math.min(items.length - 1, sel + dir)));
      }
      if (e.key === "Escape") {
        e.preventDefault();
        s.setUi(tabId, { rebase: null });
      }
    };
    panel.addEventListener("keydown", onKey);
    return () => panel.removeEventListener("keydown", onKey);
  });

  const kept = items.filter((i) => i.action !== "drop").length;
  const bottomSquash = items.length > 0 && items[items.length - 1].action === "squash";
  const start = async () => {
    setBusy(true);
    await runPlan(tabId, path, plan);
    setBusy(false);
  };

  return (
    <section ref={root} aria-label={cherry ? "Interactive Cherry Pick" : "Interactive Rebase"} className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center gap-2 h-[34px] pl-3 pr-2 bg-toolbar/60 border-b border-black/30 shrink-0">
        <span className="text-[13.5px] text-white font-semibold whitespace-nowrap">{cherry ? "Interactive Cherry Pick" : "Interactive Rebase"}</span>
        <span className="text-[12.5px] text-fg2 truncate flex-1">
          {cherry ? `Cherry picking ${items.length} commits onto ${plan.onto}` : `Rebasing ${items.length} commits onto ${plan.onto === "--root" ? "the start of history" : plan.onto}`}
        </span>
        <button ref={closeButton} onClick={() => s.setUi(tabId, { rebase: null })} className="text-fg2 hover:text-white p-1" title="Cancel (Esc)">
          <X size={16} />
        </button>
      </div>
      <div className="px-3 py-1.5 text-[11.5px] text-fg3 border-b border-white/5 shrink-0">
        Newest at the top. Pick an action for each commit (keys P, R, S, D), drag rows or use Ctrl+↑ ↓ to reorder. Squash melds a commit into the one below it.
      </div>
      <div className="flex-1 overflow-y-auto">
        {items.map((it, i) => {
          const step = STEPS.find((x) => x.step === it.action)!;
          const dropped = it.action === "drop";
          return (
            <div
              key={it.sha}
              draggable={editing !== i}
              onDragStart={() => setDragFrom(i)}
              onDragEnd={() => setDragFrom(null)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={() => {
                if (dragFrom === null || dragFrom === i) return;
                const out = [...items];
                const [m] = out.splice(dragFrom, 1);
                out.splice(i, 0, m);
                update(out);
                setSel(i);
                setDragFrom(null);
              }}
              className={cn("flex flex-col border-b border-white/5", sel === i ? "bg-selrow" : "hover:bg-hoverrow")}
            >
              <div className="flex items-center gap-2 h-[34px] pl-1.5 pr-3">
                <GripVertical size={14} className="text-fg3 cursor-grab shrink-0" />
                <div className="flex rounded-[3px] overflow-hidden border border-white/10 shrink-0">
                  {STEPS.map((x) => (
                    <button
                      key={x.step}
                      title={`${x.label} (${x.key})`}
                      onClick={() => (setSel(i), setAction(i, x.step))}
                      className={cn("w-[22px] h-[20px] text-[11px] font-bold", it.action === x.step ? "text-white" : "text-fg3 hover:text-white")}
                      style={{ background: it.action === x.step ? x.color : undefined }}
                    >
                      {x.key}
                    </button>
                  ))}
                </div>
                <button type="button" ref={i === 0 ? firstItem : undefined} data-rebase-item={i} aria-pressed={sel === i} onFocus={() => setSel(i)} onClick={() => setSel(i)} className="flex h-full flex-1 min-w-0 items-center gap-2 text-left">
                  <span className="text-[11px] font-semibold w-[50px] shrink-0" style={{ color: step.color }}>
                    {step.label}
                  </span>
                  <Avatar name={it.author} email={it.email} size={20} />
                  <span className={cn("truncate flex-1 text-[13px]", dropped ? "line-through text-fg3" : "text-white/90")}>
                    {it.action === "reword" ? it.message.split("\n")[0] : it.summary}
                  </span>
                  <span className="mono text-[11px] text-fg3 shrink-0">{it.sha.slice(0, n)}</span>
                </button>
                <button title="Move up (Ctrl+↑)" className="text-fg3 hover:text-white" onClick={() => move(i, -1)}>
                  <ArrowUp size={13} />
                </button>
                <button title="Move down (Ctrl+↓)" className="text-fg3 hover:text-white" onClick={() => move(i, 1)}>
                  <ArrowDown size={13} />
                </button>
              </div>
              {editing === i && (
                <div className="flex flex-col gap-2 pl-[38px] pr-3 pb-3">
                  <textarea
                    ref={message}
                    aria-label="Edit commit message"
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    className="h-[90px] bg-field border border-white/10 rounded-[3px] px-2 py-1.5 text-[13px] text-white outline-none focus:border-accent resize-none"
                  />
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="primary"
                      disabled={!draft.trim()}
                      onClick={() => {
                        update(items.map((x, k) => (k === i ? { ...x, message: draft.trim() } : x)));
                        setEditing(null);
                        select(i);
                      }}
                    >
                      Update Message
                    </Button>
                    <Button size="sm" onClick={() => {
                      setEditing(null);
                      select(i);
                    }}>
                      Cancel
                    </Button>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
      <div className="flex items-center gap-2 px-3 py-2.5 border-t border-white/10 bg-panel shrink-0">
        <span className="text-[12px] text-fg3 flex-1">
          {kept} of {items.length} commits kept{bottomSquash ? " · the bottom commit has nothing to squash into, so it will be picked" : ""}
        </span>
        <Button size="sm" onClick={() => s.setUi(tabId, { rebase: { ...plan, items: plan.original.map((x) => ({ ...x })) } })}>
          Reset
        </Button>
        <Button size="sm" variant="danger" onClick={() => s.setUi(tabId, { rebase: null })}>
          {cherry ? "Cancel Cherry Pick" : "Cancel Rebase"}
        </Button>
        <Button size="sm" variant="success" disabled={busy || kept === 0 || editing !== null} onClick={start}>
          {busy ? "Working…" : cherry ? "Start Cherry Pick" : "Start Rebase"}
        </Button>
      </div>
    </section>
  );
}
