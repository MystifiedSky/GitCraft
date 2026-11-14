import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { FileChange } from "@/lib/api";

const ROW = 24;
export function fileWindow(count: number, top: number, height: number) {
  const start = Math.min(count, Math.max(0, Math.floor(top / ROW) - 12));
  return { start, end: Math.min(count, Math.ceil((Math.max(0, top) + height) / ROW) + 12) };
}

/** Keep path lists bounded while preserving the surrounding panel's scrollbar. */
export function VirtualFileRows({ files, render, activeFile }: { files: FileChange[]; render: (file: FileChange) => ReactNode; activeFile?: string }) {
  const root = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLElement | null>(null);
  const [viewport, setViewport] = useState({ top: 0, height: 600 });
  const measure = useCallback(() => {
    const element = root.current;
    if (!element) return;
    let parent = element.parentElement;
    while (parent && !/auto|scroll/.test(getComputedStyle(parent).overflowY)) parent = parent.parentElement;
    scroller.current = parent;
    if (!parent) return;
    const top = Math.max(0, parent.getBoundingClientRect().top - element.getBoundingClientRect().top);
    const height = parent.clientHeight || 600;
    setViewport((old) => old.top === top && old.height === height ? old : { top, height });
  }, []);
  useLayoutEffect(() => { measure(); });
  useEffect(() => {
    measure();
    const parent = scroller.current;
    parent?.addEventListener("scroll", measure, { passive: true });
    const observer = new ResizeObserver(measure);
    if (parent) observer.observe(parent);
    if (root.current) observer.observe(root.current);
    return () => { parent?.removeEventListener("scroll", measure); observer.disconnect(); };
  }, [measure]);
  const range = fileWindow(files.length, viewport.top, viewport.height);
  const focusRow = (index: number) => {
    const element = root.current, parent = scroller.current;
    if (!element || !parent) return;
    parent.scrollTop += element.getBoundingClientRect().top - parent.getBoundingClientRect().top + index * ROW;
    measure();
    requestAnimationFrame(() => element.querySelector<HTMLButtonElement>(`[data-file-row="${index}"] button`)?.focus());
  };
  useEffect(() => {
    const index = files.findIndex((file) => file.path === activeFile);
    if (index >= 0 && (index < range.start || index >= range.end)) focusRow(index);
  }, [activeFile]);
  return <div ref={root} role="list" style={{ height: files.length * ROW, position: "relative" }} onKeyDown={(event) => {
    const row = (event.target as HTMLElement).closest<HTMLElement>("[data-file-row]");
    if (!row) return;
    const index = Number(row.dataset.fileRow);
    let next = event.key === "ArrowDown" ? index + 1 : event.key === "ArrowUp" ? index - 1 : event.key === "Home" ? 0 : event.key === "End" ? files.length - 1 : null;
    if (event.key === "Tab") {
      const buttons = [...row.querySelectorAll("button")];
      if (!event.shiftKey && index === range.end - 1 && event.target === buttons.at(-1)) next = index + 1;
      if (event.shiftKey && index === range.start && event.target === buttons[0]) next = index - 1;
    }
    if (next !== null && next >= 0 && next < files.length) { event.preventDefault(); focusRow(next); }
  }}>
    {files.slice(range.start, range.end).map((file, offset) => <div role="listitem" key={file.path} data-file-row={range.start + offset} style={{ position: "absolute", top: (range.start + offset) * ROW, width: "100%", height: ROW }}>{render(file)}</div>)}
  </div>;
}
