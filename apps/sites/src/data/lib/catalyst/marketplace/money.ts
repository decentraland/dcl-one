const WEI = 1e18;

const PRICE_SENTINEL_WEI = 2n ** 248n;

export function weiToMana(wei: string): number {
  try {
    return Number(BigInt(wei)) / WEI;
  } catch {
    const n = Number(wei);
    return Number.isFinite(n) ? n / WEI : 0;
  }
}

export function weiToManaOrNull(wei: string | null | undefined): number | null {
  if (!wei) return null;
  const mana = weiToMana(wei);
  return mana > 0 ? mana : null;
}

export function formatMana(wei: string | null | undefined): string | null {
  if (!wei) return null;
  let n: number;
  try {
    const v = BigInt(wei);
    if (v >= PRICE_SENTINEL_WEI) return null;
    n = Number(v) / WEI;
  } catch {
    const parsed = Number(wei);
    if (!Number.isFinite(parsed) || parsed >= Number(PRICE_SENTINEL_WEI)) return null;
    n = parsed / WEI;
  }
  if (!(n > 0)) return null;
  return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}
