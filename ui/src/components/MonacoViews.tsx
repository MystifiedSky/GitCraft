import { useEffect, useRef } from "react";
import { EDITOR_FONT, languageFor, monaco } from "@/lib/monaco";

const baseOptions: monaco.editor.IEditorOptions = {
  fontFamily: EDITOR_FONT,
  fontSize: 12.5,
  lineHeight: 19,
  minimap: { enabled: false },
  scrollBeyondLastLine: false,
  automaticLayout: true,
  renderLineHighlight: "line",
  smoothScrolling: true,
  stickyScroll: { enabled: false },
  scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10 },
};

export interface DiffHandle {
  next: () => void;
  prev: () => void;
}

/** Inline or side-by-side diff of two texts (Monaco). The right side can be edited. */
export function MonacoDiff({
  original,
  modified,
  file,
  sideBySide,
  wordWrap,
  ignoreWhitespace,
  editable,
  onChange,
  onSave,
  handle,
}: {
  original: string;
  modified: string;
  file: string;
  sideBySide: boolean;
  wordWrap: boolean;
  ignoreWhitespace: boolean;
  editable: boolean;
  onChange?: (v: string) => void;
  onSave?: () => void;
  handle?: React.MutableRefObject<DiffHandle | null>;
}) {
  const host = useRef<HTMLDivElement>(null);
  const ed = useRef<monaco.editor.IStandaloneDiffEditor | null>(null);
  const cb = useRef({ onChange, onSave });
  cb.current = { onChange, onSave };

  useEffect(() => {
    const e = monaco.editor.createDiffEditor(host.current!, {
      ...baseOptions,
      theme: "gitcraft-dark",
      readOnly: true,
      originalEditable: false,
      renderOverviewRuler: true,
      enableSplitViewResizing: true,
      useInlineViewWhenSpaceIsLimited: false,
      hideUnchangedRegions: { enabled: false },
    } as monaco.editor.IStandaloneDiffEditorConstructionOptions);
    ed.current = e;
    const lang = languageFor(file);
    e.setModel({ original: monaco.editor.createModel(original, lang), modified: monaco.editor.createModel(modified, lang) });
    const sub = e.getModifiedEditor().onDidChangeModelContent(() => cb.current.onChange?.(e.getModifiedEditor().getValue()));
    e.getModifiedEditor().addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => cb.current.onSave?.());
    if (handle) handle.current = { next: () => e.goToDiff("next"), prev: () => e.goToDiff("previous") };
    return () => {
      sub.dispose();
      const m = e.getModel();
      e.dispose();
      m?.original.dispose();
      m?.modified.dispose();
      if (handle) handle.current = null;
    };
    // Recreate only when the file changes; texts are updated below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file]);

  useEffect(() => {
    const m = ed.current?.getModel();
    if (!m) return;
    if (m.original.getValue() !== original) m.original.setValue(original);
    if (m.modified.getValue() !== modified) m.modified.setValue(modified);
  }, [original, modified]);

  useEffect(() => {
    ed.current?.updateOptions({ renderSideBySide: sideBySide, ignoreTrimWhitespace: ignoreWhitespace, wordWrap: wordWrap ? "on" : "off" } as monaco.editor.IDiffEditorOptions);
    ed.current?.getModifiedEditor().updateOptions({ readOnly: !editable, wordWrap: wordWrap ? "on" : "off" });
    ed.current?.getOriginalEditor().updateOptions({ wordWrap: wordWrap ? "on" : "off" });
  }, [sideBySide, ignoreWhitespace, wordWrap, editable]);

  return <div ref={host} className="h-full w-full" />;
}

/** One file in the editor (File View); editable for working-folder files. */
export function MonacoFile({
  value,
  file,
  readOnly,
  wordWrap,
  onChange,
  onSave,
}: {
  value: string;
  file: string;
  readOnly: boolean;
  wordWrap: boolean;
  onChange?: (v: string) => void;
  onSave?: () => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const ed = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const cb = useRef({ onChange, onSave });
  cb.current = { onChange, onSave };

  useEffect(() => {
    const e = monaco.editor.create(host.current!, {
      ...baseOptions,
      theme: "gitcraft-dark",
      model: monaco.editor.createModel(value, languageFor(file)),
      readOnly,
    });
    ed.current = e;
    const sub = e.onDidChangeModelContent(() => cb.current.onChange?.(e.getValue()));
    e.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => cb.current.onSave?.());
    return () => {
      sub.dispose();
      const m = e.getModel();
      e.dispose();
      m?.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file]);

  useEffect(() => {
    const e = ed.current;
    if (e && e.getValue() !== value && readOnly) e.setValue(value);
  }, [value, readOnly]);

  useEffect(() => {
    ed.current?.updateOptions({ readOnly, wordWrap: wordWrap ? "on" : "off" });
  }, [readOnly, wordWrap]);

  return <div ref={host} className="h-full w-full" />;
}
