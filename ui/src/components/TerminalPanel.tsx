import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Plus, SquareTerminal, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";

// Colours from the original's terminal theme (terminal__* in the dark theme file).
const THEME = {
  background: "#000000",
  foreground: "#FFFFFF",
  cursor: "#FFFFFF",
  cursorAccent: "#000000",
  selectionBackground: "#304676",
  black: "#000000",
  red: "#F24A4A",
  green: "#0DBC79",
  yellow: "#E5E510",
  blue: "#4A98EE",
  magenta: "#E063E0",
  cyan: "#11A8CD",
  white: "#E5E5E5",
  brightBlack: "#686868",
  brightRed: "#FF5656",
  brightGreen: "#23D18B",
  brightYellow: "#F5F543",
  brightBlue: "#51A4FF",
  brightMagenta: "#D670D6",
  brightCyan: "#29B8DB",
  brightWhite: "#E5E5E5",
};

interface Tab {
  key: number;
  id: number | null;
  title: string;
  /** A command typed into the shell once it starts (coding agent sessions). */
  run?: string;
}

/** Open a new in-app terminal tab that runs `command` (shows the terminal panel). */
export function runInTerminal(command: string, title: string) {
  useStore.getState().setSettings({ showTerminal: true });
  setTimeout(() => window.dispatchEvent(new CustomEvent("gitcraft-terminal-run", { detail: { command, title } })), 50);
}

/** In-app terminal under the graph, with terminal tabs (Ctrl+` toggles it). */
export function TerminalPanel({ cwd }: { cwd: string }) {
  const s = useStore();
  const [tabs, setTabs] = useState<Tab[]>([{ key: 1, id: null, title: "Terminal 1" }]);
  const [active, setActive] = useState(1);
  const add = () => {
    const key = Math.max(0, ...tabs.map((t) => t.key)) + 1;
    setTabs([...tabs, { key, id: null, title: `Terminal ${key}` }]);
    setActive(key);
  };
  useEffect(() => {
    const on = (e: Event) => {
      const { command, title } = (e as CustomEvent<{ command: string; title: string }>).detail;
      setTabs((all) => {
        const key = Math.max(0, ...all.map((t) => t.key)) + 1;
        setActive(key);
        return [...all, { key, id: null, title, run: command }];
      });
    };
    window.addEventListener("gitcraft-terminal-run", on);
    return () => window.removeEventListener("gitcraft-terminal-run", on);
  }, []);
  const close = (key: number) => {
    const t = tabs.find((x) => x.key === key);
    if (t?.id) invoke("term_close", { id: t.id });
    const rest = tabs.filter((x) => x.key !== key);
    if (!rest.length) return s.setSettings({ showTerminal: false });
    setTabs(rest);
    if (active === key) setActive(rest[rest.length - 1].key);
  };

  return (
    <div className="flex flex-col bg-black border-t border-white/15 shrink-0" style={{ height: s.settings.terminalHeight }}>
      <div
        className="h-[4px] cursor-row-resize hover:bg-accent shrink-0"
        onPointerDown={(e) => {
          let y = e.clientY;
          const el = e.currentTarget;
          el.setPointerCapture(e.pointerId);
          const move = (ev: PointerEvent) => {
            const h = Math.min(700, Math.max(100, useStore.getState().settings.terminalHeight - (ev.clientY - y)));
            y = ev.clientY;
            s.setSettings({ terminalHeight: h });
          };
          const up = () => (el.removeEventListener("pointermove", move), el.removeEventListener("pointerup", up));
          el.addEventListener("pointermove", move);
          el.addEventListener("pointerup", up);
        }}
      />
      <div className="flex items-center h-[26px] bg-panel border-b border-black/40 shrink-0 text-[12px]">
        <SquareTerminal size={13} className="mx-2 text-fg2" />
        {tabs.map((t) => (
          <div
            key={t.key}
            onClick={() => setActive(t.key)}
            className={cn("group flex items-center gap-1 h-full pl-2.5 pr-1 border-r border-black/30 cursor-default", active === t.key ? "bg-black text-white" : "text-fg2 hover:text-white")}
          >
            {t.title}
            <button className="opacity-60 hover:opacity-100 p-0.5" onClick={(e) => (e.stopPropagation(), close(t.key))} title="Close terminal tab">
              <X size={11} />
            </button>
          </div>
        ))}
        <button className="px-2 text-fg2 hover:text-white" title="New terminal tab" onClick={add}>
          <Plus size={13} />
        </button>
        <div className="flex-1" />
        <button className="px-2 text-fg2 hover:text-white" title="Hide terminal (Ctrl+`)" onClick={() => s.setSettings({ showTerminal: false })}>
          <X size={13} />
        </button>
      </div>
      <div className="flex-1 min-h-0 relative">
        {tabs.map((t) => (
          <TermView
            key={t.key}
            cwd={cwd}
            run={t.run}
            visible={active === t.key}
            onReady={(id) => setTabs((all) => all.map((x) => (x.key === t.key ? { ...x, id } : x)))}
            onExit={() => close(t.key)}
          />
        ))}
      </div>
    </div>
  );
}

function TermView({ cwd, run, visible, onReady, onExit }: { cwd: string; run?: string; visible: boolean; onReady: (id: number) => void; onExit: () => void }) {
  // Preferences > In-App Terminal.
  const pref = useStore.getState().settings;
  const [focused, setFocused] = useState(true);
  const dim = useStore((s) => s.settings.termDim !== false);
  const host = useRef<HTMLDivElement>(null);
  const fit = useRef<FitAddon | null>(null);
  const term = useRef<Terminal | null>(null);
  const idRef = useRef<number | null>(null);
  const cb = useRef({ onReady, onExit });
  cb.current = { onReady, onExit };

  useEffect(() => {
    const t = new Terminal({
      fontFamily: `${pref.termFontFamily ? `'${pref.termFontFamily}', ` : ""}'Cascadia Mono', Consolas, monospace`,
      fontSize: pref.termFontSize || 12,
      lineHeight: pref.termLineHeight || 1.2,
      cursorStyle: pref.termCursor || "bar",
      theme: THEME,
      cursorBlink: true,
      scrollback: 5000,
      allowProposedApi: true,
    });
    const f = new FitAddon();
    t.loadAddon(f);
    t.open(host.current!);
    term.current = t;
    fit.current = f;
    try {
      f.fit();
    } catch {
      /* not visible yet */
    }
    let disposed = false;
    const unData = listen<{ id: number; data: string }>("term-data", (e) => {
      if (e.payload.id === idRef.current) t.write(e.payload.data);
    });
    const unExit = listen<number>("term-exit", (e) => {
      if (e.payload === idRef.current) cb.current.onExit();
    });
    invoke<number>("term_open", { cwd, cols: t.cols, rows: t.rows, shell: pref.termShell || null })
      .then((id) => {
        if (disposed) return invoke("term_close", { id });
        idRef.current = id;
        cb.current.onReady(id);
        if (run) setTimeout(() => invoke("term_write", { id, data: run + "\r" }), 700);
      })
      .catch((e) => t.write(`\r\nCould not start a shell: ${e}\r\n`));
    const input = t.onData((d) => idRef.current && invoke("term_write", { id: idRef.current, data: d }));
    const resize = t.onResize(({ cols, rows }) => idRef.current && invoke("term_resize", { id: idRef.current, cols, rows }));
    const ro = new ResizeObserver(() => {
      try {
        f.fit();
      } catch {
        /* hidden */
      }
    });
    ro.observe(host.current!);
    return () => {
      disposed = true;
      ro.disconnect();
      input.dispose();
      resize.dispose();
      unData.then((u) => u());
      unExit.then((u) => u());
      if (idRef.current) invoke("term_close", { id: idRef.current });
      t.dispose();
    };
  }, [cwd]);

  useEffect(() => {
    if (visible) {
      setTimeout(() => {
        try {
          fit.current?.fit();
        } catch {
          /* ignore */
        }
        term.current?.focus();
      }, 0);
    }
  }, [visible]);

  return (
    <div
      ref={host}
      onFocusCapture={() => setFocused(true)}
      onBlurCapture={() => setFocused(false)}
      className={cn("absolute inset-0 pl-2 pt-1 transition-opacity", !visible && "invisible", dim && !focused && "opacity-60")}
    />
  );
}
