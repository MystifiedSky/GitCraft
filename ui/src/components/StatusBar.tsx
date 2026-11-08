import { Keyboard, List, Loader2, Map as MapIcon, Rocket, ZoomIn } from "lucide-react";
import { useStore } from "@/lib/store";
import { resetZoom } from "@/lib/zoom";

export function StatusBar() {
  const s = useStore();
  return (
    <div className="h-[26px] bg-panel flex items-center shrink-0 border-t border-black/30 text-[13px] text-white/90">
      <div className="flex items-center gap-2 pl-[22px] flex-1 min-w-0">
        {s.busy ? <Loader2 size={14} className="spin text-link" /> : <Rocket size={14} className="text-white/80" />}
        <span className="truncate">{s.busy ?? "Ready"}</span>
      </div>
      <SB title="Activity log" onClick={() => s.setDialog({ kind: "log" })}>
        <List size={15} />
      </SB>
      <SB title="Keyboard shortcuts" onClick={() => s.setDialog({ kind: "shortcuts" })}>
        <Keyboard size={15} />
      </SB>
      <SB title="Feature plan" onClick={() => s.setDialog({ kind: "plan" })}>
        <MapIcon size={15} />
      </SB>
      <SB title="Reset zoom" onClick={resetZoom}>
        <ZoomIn size={15} />
        <span className="ml-1.5">{Math.round(s.settings.zoom * 100)}%</span>
      </SB>
      <button className="px-3 text-link hover:underline" onClick={() => s.setDialog({ kind: "shortcuts" })}>
        Help
      </button>
      <span className="mx-2 px-1.5 h-[20px] flex items-center rounded-[3px] border border-white/25 mono text-[12px] tracking-wide">{s.version}</span>
    </div>
  );
}

function SB({ children, title, onClick }: { children: React.ReactNode; title: string; onClick: () => void }) {
  return (
    <button title={title} onClick={onClick} className="h-full px-[9px] flex items-center text-white/85 hover:text-white hover:bg-white/5">
      {children}
    </button>
  );
}
