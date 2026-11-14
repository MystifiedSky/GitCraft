import { useEffect, useState } from "react";
import { authorColor, initials } from "@/lib/utils";
import { useStore } from "@/lib/store";

/** Gravatar address for an e-mail (SHA-256 of the trimmed, lower-case address). */
const avatarCache = new Map<string, Promise<string>>();
function gravatar(email: string, size: number): Promise<string> {
  const key = `${email}|${size}`;
  let p = avatarCache.get(key);
  if (!p) {
    p = crypto.subtle.digest("SHA-256", new TextEncoder().encode(email.trim().toLowerCase())).then((h) => {
      const hex = [...new Uint8Array(h)].map((b) => b.toString(16).padStart(2, "0")).join("");
      return `https://gravatar.com/avatar/${hex}?s=${size * 2}&d=404`;
    });
    avatarCache.set(key, p);
  }
  return p;
}
const missing = new Set<string>();

/** GitCraft's own mark: two lanes joining, drawn with simple shapes. */
export function AppMark({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16">
      <rect x="0.5" y="0.5" width="15" height="15" rx="3.5" fill="#1c1e23" stroke="#15a0bf" strokeOpacity="0.6" />
      <path d="M5.5 4 V12 M10.5 5 Q10.5 9 5.8 10" stroke="#15a0bf" strokeWidth="1.6" fill="none" />
      <circle cx="5.5" cy="4" r="1.7" fill="#15a0bf" />
      <circle cx="5.5" cy="12" r="1.7" fill="#15a0bf" />
      <circle cx="10.5" cy="5" r="1.7" fill="#c517b6" />
    </svg>
  );
}

export function Avatar({ name, email, size = 22 }: { name: string; email: string; size?: number }) {
  // UI Customization > "Display author initials ... instead of avatars" (off by default: avatars where the author has one).
  const initialsOnly = useStore((s) => s.settings.authorInitials);
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setSrc(null);
    if (!initialsOnly && email && !missing.has(email)) gravatar(email, size).then((u) => live && setSrc(u), () => undefined);
    return () => {
      live = false;
    };
  }, [email, size, initialsOnly]);
  if (src)
    return (
      <img
        src={src}
        alt={name}
        width={size}
        height={size}
        className="rounded-full shrink-0 object-cover"
        style={{ width: size, height: size }}
        onError={() => {
          missing.add(email);
          setSrc(null);
        }}
      />
    );
  return (
    <span
      className="inline-flex items-center justify-center rounded-full text-white font-semibold shrink-0"
      style={{ width: size, height: size, background: authorColor(email || name), fontSize: size * 0.42 }}
    >
      {initials(name)}
    </span>
  );
}

/** Small square "profile" picture made from shapes (no artwork copied). */
export const PROFILE_COLORS = ["#7bd938", "#2ec4b6", "#b57bff", "#ff9f43"];

export function ProfileMark({ color = PROFILE_COLORS[0], size = 18 }: { color?: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 9 9" shapeRendering="crispEdges">
      <rect width="9" height="9" fill="transparent" />
      <rect x="2" y="1" width="5" height="1" fill={color} />
      <rect x="1" y="2" width="7" height="4" fill={color} />
      <rect x="2" y="3" width="1" height="1" fill="#1c1e23" />
      <rect x="6" y="3" width="1" height="1" fill="#1c1e23" />
      <rect x="1" y="6" width="2" height="2" fill={color} />
      <rect x="6" y="6" width="2" height="2" fill={color} />
    </svg>
  );
}

export function StatusIcon({ status }: { status: string }) {
  const map: Record<string, [string, string]> = {
    M: ["#de9b43", "✎"],
    A: ["#5cb85c", "+"],
    "?": ["#5cb85c", "+"],
    D: ["#d9413d", "−"],
    R: ["#4d88ff", "→"],
    U: ["#d9413d", "!"],
  };
  const [c, ch] = map[status] ?? ["#999", "•"];
  return (
    <span className="inline-flex items-center justify-center w-[14px] h-[14px] rounded-[3px] text-[10px] font-bold shrink-0" style={{ background: c + "33", color: c }}>
      {ch}
    </span>
  );
}
