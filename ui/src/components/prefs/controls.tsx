import { createContext, useContext, useEffect, useId, useState, type ReactNode } from "react";
import { FolderOpen } from "lucide-react";
import { cn } from "@/lib/utils";
import { tools } from "@/lib/integrations";
import { api } from "@/lib/api";

const SettingContext = createContext<{ label: string; description?: string; invalid: boolean } | null>(null);
function useSettingLabel(explicit?: string) {
  const context=useContext(SettingContext);
  return { ...(explicit ? {"aria-label":explicit} : context ? {"aria-labelledby":context.label} : {}), "aria-describedby":context?.description, "aria-invalid":context?.invalid || undefined };
}

export const field = "h-[28px] px-2 rounded-[3px] bg-field border border-white/10 text-fg text-[13px] outline-none focus:border-accent min-w-0";

/** A section title: bold h3. */
export function H({ children, sub }: { children: ReactNode; sub?: boolean }) {
  return <h3 className={cn("font-semibold text-white/95", sub ? "text-[14px] mt-6 mb-2" : "text-[18px] mb-4")}>{children}</h3>;
}

/** One "form-group setting-row": label on the left, control on the right, help / warning text under it. */
export function Row({ label, help, warning, error, children, wide }: { label: ReactNode; help?: ReactNode; warning?: ReactNode; error?: ReactNode; children: ReactNode; wide?: boolean }) {
  const id=useId();
  const description=[help&&`${id}-help`,warning&&`${id}-warning`,error&&`${id}-error`].filter(Boolean).join(" ") || undefined;
  return (
    <div className="flex gap-4 py-2 border-b border-white/[0.04]">
      <div id={`${id}-label`} className="w-[260px] shrink-0 pt-1 text-[13px] text-fg">{label}</div>
      <div className={cn("flex-1 min-w-0", wide ? "" : "max-w-[460px]")}>
        <SettingContext.Provider value={{label:`${id}-label`,description,invalid:!!error}}><div className="flex items-center gap-2 min-h-[28px]">{children}</div></SettingContext.Provider>
        {help && <div id={`${id}-help`} className="text-[12px] text-fg3 mt-1 leading-snug whitespace-pre-wrap">{help}</div>}
        {warning && <div id={`${id}-warning`} className="text-[12px] text-warn mt-1 leading-snug">{warning}</div>}
        {error && <div id={`${id}-error`} className="text-[12px] text-[#ef7a76] mt-1 leading-snug">{error}</div>}
      </div>
    </div>
  );
}

export function Toggle({ value, onChange, disabled }: { value: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  const a11y=useSettingLabel();
  return <input {...a11y} type="checkbox" disabled={disabled} checked={value} onChange={(e) => onChange(e.target.checked)} className="w-[15px] h-[15px] accent-[#80B1EA]" />;
}

/** A text box that saves when you leave it or press Enter. */
export function Text({ value, onSave, placeholder, password, className, mono, label }: { value: string; onSave: (v: string) => void; placeholder?: string; password?: boolean; className?: string; mono?: boolean; label?: string }) {
  const a11y=useSettingLabel(label);
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  const save = () => v !== value && onSave(v);
  return (
    <input
      type={password ? "password" : "text"}
      className={cn(field, "flex-1", mono && "mono", className)}
      value={v}
      placeholder={placeholder}
      {...a11y}
      onChange={(e) => setV(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === "Enter" && save()}
    />
  );
}

export function Area({ value, onSave, placeholder, rows = 4, mono }: { value: string; onSave: (v: string) => void; placeholder?: string; rows?: number; mono?: boolean }) {
  const a11y=useSettingLabel();
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return (
    <textarea
      {...a11y}
      rows={rows}
      className={cn(field, "flex-1 h-auto py-1.5 resize-y", mono && "mono")}
      value={v}
      placeholder={placeholder}
      onChange={(e) => setV(e.target.value)}
      onBlur={() => v !== value && onSave(v)}
    />
  );
}

export function Num({ value, onSave, min, max, step = 1, width = 90 }: { value: number; onSave: (v: number) => void; min?: number; max?: number; step?: number; width?: number }) {
  const a11y=useSettingLabel();
  const [v, setV] = useState(String(value));
  useEffect(() => setV(String(value)), [value]);
  const save = () => {
    let n = Number(v);
    if (!Number.isFinite(n)) n = value;
    if (min !== undefined) n = Math.max(min, n);
    if (max !== undefined) n = Math.min(max, n);
    setV(String(n));
    if (n !== value) onSave(n);
  };
  return <input {...a11y} type="number" step={step} min={min} max={max} className={field} style={{ width }} value={v} onChange={(e) => setV(e.target.value)} onBlur={save} onKeyDown={(e) => e.key === "Enter" && save()} />;
}

export function Select<T extends string>({ value, onChange, options, className, label }: { value: T; onChange: (v: T) => void; options: [T, string][]; className?: string; label?: string }) {
  const a11y=useSettingLabel(label);
  return (
    <select {...a11y} className={cn(field, "flex-1", className)} value={value} onChange={(e) => onChange(e.target.value as T)}>
      {options.map(([k, l]) => (
        <option key={k} value={k}>
          {l}
        </option>
      ))}
    </select>
  );
}

/** A path with Browse (file or folder). */
export function PathField({ value, onSave, placeholder, folder, title }: { value: string; onSave: (v: string) => void; placeholder?: string; folder?: boolean; title: string }) {
  const browseLabel=useSettingLabel(`Browse: ${title}`);
  return (
    <>
      <Text value={value} onSave={onSave} placeholder={placeholder} mono label={title} />
      <button
        {...browseLabel}
        className="h-[28px] px-2 rounded-[3px] bg-white/[0.06] border border-white/10 text-[12.5px] text-fg hover:bg-white/10 flex items-center gap-1 shrink-0"
        onClick={async () => {
          const p = folder ? await api.pickFolder(title) : await tools.pickFile(title);
          if (p) onSave(p);
        }}
      >
        <FolderOpen size={13} /> Browse
      </button>
    </>
  );
}

export function Btn({ children, onClick, danger, primary, disabled, title }: { children: ReactNode; onClick: () => void; danger?: boolean; primary?: boolean; disabled?: boolean; title?: string }) {
  return (
    <button
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "h-[28px] px-3 rounded-[3px] border text-[12.5px] shrink-0 disabled:opacity-40",
        danger ? "border-bad/70 text-[#ef7a76] hover:bg-bad/15" : primary ? "border-accent bg-accent/25 text-white hover:bg-accent/35" : "border-white/10 bg-white/[0.06] text-fg hover:bg-white/10",
      )}
    >
      {children}
    </button>
  );
}

export function Note({ children }: { children: ReactNode }) {
  return <p className="text-[12.5px] text-fg2 leading-relaxed mb-3 max-w-[760px]">{children}</p>;
}
