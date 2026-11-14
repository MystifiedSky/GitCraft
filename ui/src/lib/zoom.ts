import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useStore } from "./store";

/** Apply a zoom factor (100 % = 1). */
export function setZoom(z: number) {
  useStore.getState().setSettings({ zoom: z });
  getCurrentWebview()
    .setZoom(z)
    .catch(() => {
      document.documentElement.style.zoom = String(z);
    });
}

/** Zoom in / out moves to the next entry of ZOOM_FACTORS (80 % … 300 %). */
export function stepZoom(dir: 1 | -1) {
  const { metrics, settings } = useStore.getState();
  const levels = [...metrics.zoom_factors].sort((a, b) => a - b);
  const cur = settings.zoom;
  const next = dir === 1 ? levels.find((l) => l > cur + 1e-6) : [...levels].reverse().find((l) => l < cur - 1e-6);
  if (next !== undefined) setZoom(next);
}

export function resetZoom() {
  setZoom(useStore.getState().metrics.default_zoom);
}
