// Monaco (the code editor) bundled locally — no network needed.
import * as monaco from "monaco-editor";
import { registerEditorTheme } from "./editor-theme";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

declare global {
  interface Window {
    MonacoEnvironment?: monaco.Environment;
  }
}

// Syntax colouring only needs the basic editor worker.
self.MonacoEnvironment = { getWorker: () => new EditorWorker() };

// Dark theme with GitCraft's colours (added-line / deleted-line = green / red at 20 %).
monaco.editor.defineTheme("gitcraft-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#1c1e23",
    "editorGutter.background": "#1c1e23",
    "editorLineNumber.foreground": "#ffffff55",
    "editor.lineHighlightBackground": "#ffffff08",
    "diffEditor.insertedLineBackground": "#5cb85c33",
    "diffEditor.removedLineBackground": "#d9413d33",
    "diffEditor.insertedTextBackground": "#5cb85c40",
    "diffEditor.removedTextBackground": "#d9413d40",
    "diffEditorGutter.insertedLineBackground": "#5cb85c40",
    "diffEditorGutter.removedLineBackground": "#d9413d40",
    "editorOverviewRuler.border": "#00000000",
    "scrollbarSlider.background": "#ffffff26",
  },
});

monaco.editor.defineTheme("gitcraft-light", {
  base: "vs",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#fcfcfc",
    "editorGutter.background": "#fcfcfc",
    "diffEditor.insertedLineBackground": "#5cb85c33",
    "diffEditor.removedLineBackground": "#d9413d33",
  },
});

/** The editor theme for GitCraft's current theme. */
export function editorTheme(): string {
  return (document.documentElement.dataset.theme ?? "dark").startsWith("light") ? "gitcraft-light" : "gitcraft-dark";
}

/** Monaco language id for a file name, from the languages Monaco knows. */
export function languageFor(file: string): string {
  file = file.split("#")[0]; // views add "#<sha>" to tell versions apart
  const name = file.split("/").pop()!.toLowerCase();
  const ext = name.includes(".") ? "." + name.split(".").pop() : name;
  for (const l of monaco.languages.getLanguages()) {
    if (l.extensions?.some((e) => e.toLowerCase() === ext) || l.filenames?.some((f) => f.toLowerCase() === name)) return l.id;
  }
  return "plaintext";
}

registerEditorTheme(() => monaco.editor.setTheme(editorTheme()));

export const EDITOR_FONT = "'Cascadia Mono', Consolas, 'Courier New', monospace";

export { monaco };
