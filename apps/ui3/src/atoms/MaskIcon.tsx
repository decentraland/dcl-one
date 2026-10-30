import { isValidElement, type CSSProperties, type ReactElement, type ReactNode } from "react";
import "./maskicon.css";

const ATTR: Record<string, string> = {
  strokeWidth: "stroke-width",
  strokeLinecap: "stroke-linecap",
  strokeLinejoin: "stroke-linejoin",
  strokeDasharray: "stroke-dasharray",
  strokeMiterlimit: "stroke-miterlimit",
  strokeOpacity: "stroke-opacity",
  fillOpacity: "fill-opacity",
  fillRule: "fill-rule",
  clipRule: "clip-rule",
  viewBox: "viewBox",
};
const MONO = new Set(["currentColor", "none"]);
const cache = new WeakMap<object, string | null>();

function markup(node: ReactNode): string | null {
  if (node == null || node === false || node === true) return "";
  if (Array.isArray(node)) {
    let out = "";
    for (const child of node) {
      const m = markup(child);
      if (m === null) return null;
      out += m;
    }
    return out;
  }
  if (typeof node === "string" || typeof node === "number") return "";
  if (!isValidElement(node) || typeof node.type !== "string") return null;
  const { children, className: _className, style, ...props } = node.props as Record<string, unknown> & { children?: ReactNode };
  if (style) return null;
  let attrs = "";
  for (const [key, value] of Object.entries(props)) {
    if (value == null || value === false || key.startsWith("aria-") || key === "role" || key === "focusable") continue;
    if (typeof value !== "string" && typeof value !== "number") return null;
    if ((key === "fill" || key === "stroke") && !MONO.has(String(value))) return null;
    const name = ATTR[key] ?? key;
    const text = String(value) === "currentColor" ? "#000" : String(value).replace(/&/g, "&amp;").replace(/"/g, "&quot;");
    attrs += ` ${name}="${text}"`;
  }
  const inner = markup(children);
  if (inner === null) return null;
  return `<${node.type}${attrs}>${inner}</${node.type}>`;
}

export function svgMask(icon: ReactElement): string | null {
  if (cache.has(icon)) return cache.get(icon)!;
  let result: string | null = null;
  if (icon.type === "svg") {
    const body = markup(icon);
    if (body) result = `url("data:image/svg+xml,${encodeURIComponent(body.replace(/^<svg/, `<svg xmlns="http://www.w3.org/2000/svg"`))}")`;
  }
  cache.set(icon, result);
  return result;
}

export function flatIcon(icon: ReactNode, className = "mi"): ReactNode {
  if (!isValidElement(icon) || icon.type !== "svg") return icon;
  const mask = svgMask(icon);
  if (!mask) return icon;
  const { width, height } = icon.props as { width?: number | string; height?: number | string };
  const px = (v: number | string | undefined) => (v == null ? undefined : /^\d+(\.\d+)?$/.test(String(v)) ? `${v}px` : String(v));
  const style = { "--mi": mask, width: px(width), height: px(height ?? width) } as CSSProperties;
  return <span className={className} aria-hidden="true" style={style} />;
}
