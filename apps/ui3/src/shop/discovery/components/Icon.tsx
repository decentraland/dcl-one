// from: shop@94584a5c:app/src/components/Icon/Icon.tsx

import type { ComponentPropsWithoutRef, CSSProperties } from "react";

import type { IconName } from "../lib/iconName";
import "./icon.css";

const URLS = import.meta.glob<string>("../icons/*.svg", { eager: true, query: "?url&no-inline", import: "default" });

const SRC: Record<string, string> = {};
for (const [path, url] of Object.entries(URLS)) SRC[path.slice(path.lastIndexOf("/") + 1, -".svg".length)] = url;

export type { IconName };

type IconProps = { name: IconName; size?: number; color?: string } & ComponentPropsWithoutRef<"span">;

export default function Icon({ name, size, color, className, style, ...rest }: IconProps) {
  const url = SRC[name];
  const vars = {
    "--shop-icon-url": url ? `url("${url}")` : undefined,
    ...(size != null ? { width: size, height: size } : null),
    ...(color != null ? { color } : null),
    ...style,
  } as CSSProperties;
  const decorative = rest["aria-label"] == null && rest.role == null;
  return <span aria-hidden={decorative || undefined} {...rest} className={className ? `shop-ico ${className}` : "shop-ico"} style={vars} />;
}
