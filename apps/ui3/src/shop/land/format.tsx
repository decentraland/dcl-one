// Money and dates as the LAND and NAME screens print them. MANA is always its exact token amount
// (SHOP.md: a displayed conversion never changes a charge); these screens never print a credit price
// for LAND, an estate, a rental or a resale.

import ManaMark from "../../atoms/ManaMark";
import { formatMana, manaWei, type ManaWei } from "../model/money";
import "./land.css";

export function manaText(wei: bigint | string): string {
  return formatMana(manaWei(typeof wei === "string" ? BigInt(wei) : wei) as ManaWei, { group: true });
}

/**
 * The MANA mark and an exact amount; `network` picks the Ethereum or Polygon glyph. `text` is for the
 * map service's display number (the atlas tile price), which is not a charge.
 */
export function Mana({ wei, text, network = "ethereum", suffix }: { wei?: bigint | string; text?: string; network?: "ethereum" | "polygon"; suffix?: string }) {
  const value = wei !== undefined ? manaText(wei) : (text ?? "");
  return (
    <span className="shop-land-mana" title={`${value} MANA${network === "polygon" ? " on Polygon" : " on Ethereum"}`}>
      <ManaMark size={14} network={network} />
      <span className="shop-land-mana__value">{value}</span>
      {suffix ? <span className="shop-land-mana__suffix">{suffix}</span> : null}
    </span>
  );
}

const DATE = new Intl.DateTimeFormat("en-US", { year: "numeric", month: "long", day: "numeric" });
const SHORT = new Intl.DateTimeFormat("en-US", { month: "short", day: "2-digit" });

export function dateText(ms: number): string {
  return DATE.format(new Date(ms));
}

export function shortDateText(ms: number): string {
  return SHORT.format(new Date(ms));
}

const RELATIVE = new Intl.RelativeTimeFormat("en-US", { numeric: "auto" });

export function relativeText(ms: number, now: number = Date.now()): string {
  const days = Math.round((ms - now) / 86_400_000);
  if (Math.abs(days) >= 1) return RELATIVE.format(days, "day");
  const hours = Math.round((ms - now) / 3_600_000);
  return RELATIVE.format(hours, "hour");
}

/** Orders and listings carry expiry in seconds or milliseconds. */
export function epochMs(t: number): number {
  return t < 1e12 ? t * 1000 : t;
}
