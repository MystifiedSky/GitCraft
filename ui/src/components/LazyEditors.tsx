import { lazy, Suspense, type ComponentProps } from "react";
import type { MonacoDiff as Diff, MonacoFile as File } from "./MonacoViews";
export type { DiffHandle } from "./MonacoViews";
const DiffEditor = lazy(() => import("./MonacoViews").then((m) => ({ default: m.MonacoDiff })));
const FileEditor = lazy(() => import("./MonacoViews").then((m) => ({ default: m.MonacoFile })));
const fallback = <div role="status" className="p-4 text-fg3">Loading editor…</div>;
export function MonacoDiff(props: ComponentProps<typeof Diff>) {
  return <Suspense fallback={fallback}><DiffEditor {...props} /></Suspense>;
}
export function MonacoFile(props: ComponentProps<typeof File>) {
  return <Suspense fallback={fallback}><FileEditor {...props} /></Suspense>;
}
