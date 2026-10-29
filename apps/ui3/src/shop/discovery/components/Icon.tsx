// from: shop@94584a5c:app/src/components/Icon/Icon.tsx

import type { ComponentPropsWithoutRef, CSSProperties } from "react";

import "./icon.css";

const URLS = import.meta.glob<string>("../icons/*.svg", { eager: true, query: "?url&no-inline", import: "default" });

const SRC: Record<string, string> = {};
for (const [path, url] of Object.entries(URLS)) SRC[path.slice(path.lastIndexOf("/") + 1, -".svg".length)] = url;

export type IconName =
  | "arrow-left"
  | "arrow-right"
  | "ban"
  | "carousel-arrow"
  | "cart"
  | "cart-solid"
  | "cat-accessories"
  | "cat-earring"
  | "cat-eyebrows"
  | "cat-eyes"
  | "cat-eyewear"
  | "cat-facial-hair"
  | "cat-feet"
  | "cat-hair"
  | "cat-handwear"
  | "cat-hat"
  | "cat-head"
  | "cat-helmet"
  | "cat-lower"
  | "cat-mask"
  | "cat-mouth"
  | "cat-skins"
  | "cat-tiara"
  | "cat-top-head"
  | "cat-upper"
  | "check"
  | "chevron-down"
  | "clock"
  | "close"
  | "copy"
  | "credits"
  | "emote-dance"
  | "emote-fun"
  | "emote-greetings"
  | "emote-horror"
  | "emote-misc"
  | "emote-poses"
  | "emote-reactions"
  | "emote-stunt"
  | "external-link"
  | "eye"
  | "filter"
  | "fitting-room"
  | "gender-female"
  | "gender-male"
  | "gender-unisex"
  | "heart"
  | "heart-solid"
  | "history"
  | "info"
  | "play"
  | "play-loop"
  | "play-once"
  | "plus"
  | "props"
  | "refresh"
  | "search"
  | "smart"
  | "sound"
  | "star-rounded"
  | "utility"
  | "view-all-arrow"
  | "warning-fill";

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
