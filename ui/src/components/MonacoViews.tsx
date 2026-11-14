import { useEffect, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { EDITOR_FONT, editorTheme, languageFor, monaco } from "@/lib/monaco";
import { useStore } from "@/lib/store";

/** Preferences > Editor: font, size, line numbers, syntax highlighting, tab size, end of line. */
function useEditorPrefs() {
  return useStore(
    useShallow((s) => ({
      font: s.settings.editorFontFamily,
      size: s.settings.editorFontSize || 12,
      lines: s.settings.editorLineNumbers !== false,
      syntax: s.settings.editorSyntax !== false,
      tab: s.settings.editorTabSize || 4,
      eol: s.settings.editorEol,
    })),
  );
}

type Prefs = ReturnType<typeof useEditorPrefs>;

function prefOptions(p: Prefs): monaco.editor.IEditorOptions {
  return {
    fontFamily: p.font ? `'${p.font}', ${EDITOR_FONT}` : EDITOR_FONT,
    fontSize: p.size,
    lineHeight: Math.round(p.size * 1.5),
    lineNumbers: p.lines ? "on" : "off",
  };
}

function applyModelPrefs(m: monaco.editor.ITextModel | null | undefined, p: Prefs, file: string) {
  if (!m) return;
  m.updateOptions({ tabSize: p.tab, insertSpaces: true });
  monaco.editor.setModelLanguage(m, p.syntax ? languageFor(file) : "plaintext");
  m.setEOL(p.eol === "crlf" ? monaco.editor.EndOfLineSequence.CRLF : monaco.editor.EndOfLineSequence.LF);
}

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
  const prefs = useEditorPrefs();

  useEffect(() => {
    const e = monaco.editor.createDiffEditor(host.current!, {
      ...baseOptions,
      ...prefOptions(prefs),
      theme: editorTheme(),
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
  }, [file, sideBySide, ignoreWhitespace, wordWrap, editable]);

  useEffect(() => {
    ed.current?.updateOptions(prefOptions(prefs) as monaco.editor.IDiffEditorOptions);
    const m = ed.current?.getModel();
    applyModelPrefs(m?.original, prefs, file);
    applyModelPrefs(m?.modified, prefs, file);
  }, [prefs, file]);

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
  const prefs = useEditorPrefs();

  useEffect(() => {
    const e = monaco.editor.create(host.current!, {
      ...baseOptions,
      ...prefOptions(prefs),
      theme: editorTheme(),
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
  }, [file, readOnly, wordWrap]);

  useEffect(() => {
    ed.current?.updateOptions(prefOptions(prefs));
    applyModelPrefs(ed.current?.getModel(), prefs, file);
  }, [prefs, file]);

  return <div ref={host} className="h-full w-full" />;
}
