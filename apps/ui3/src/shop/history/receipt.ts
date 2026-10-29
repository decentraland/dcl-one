export type ReceiptLine = {
  name: string;
  thumbnail?: string;
  href?: string;
  quantity: number;
  method: "credits" | "mana";
  creator?: string;
  category?: string;
  urn?: string;
  contractAddress?: string;
  itemId?: string;
  credits?: number;
};
export type ReceiptTx = { hash: string; chainId: number; method: "credits" | "mana" };

export type SuccessNavState = {
  items: ReceiptLine[];
  txs: ReceiptTx[];
  settled?: boolean;
  creditsAdded?: number;
};

const KEY = "dcl-shop-receipt:v1";

export function saveReceipt(state: SuccessNavState): void {
  try {
    sessionStorage.setItem(KEY, JSON.stringify({ ...state, at: Date.now() }));
  } catch {
  }
}

export function loadReceipt(maxAgeMs = 30 * 60 * 1000): SuccessNavState | null {
  try {
    const raw = sessionStorage.getItem(KEY);
    const parsed = raw ? (JSON.parse(raw) as SuccessNavState & { at?: number }) : null;
    if (!parsed || !Array.isArray(parsed.items) || !Array.isArray(parsed.txs)) return null;
    if (typeof parsed.at === "number" && Date.now() - parsed.at > maxAgeMs) return null;
    return { items: parsed.items, txs: parsed.txs, settled: parsed.settled, creditsAdded: parsed.creditsAdded };
  } catch {
    return null;
  }
}

export function isSuccessNavState(v: unknown): v is SuccessNavState {
  const s = v as SuccessNavState | null;
  return !!s && Array.isArray(s.items) && Array.isArray(s.txs);
}
