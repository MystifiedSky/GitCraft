let updateTheme: (() => void) | undefined;
export function registerEditorTheme(update: () => void) { updateTheme = update; update(); }
export function updateEditorTheme() { updateTheme?.(); }
