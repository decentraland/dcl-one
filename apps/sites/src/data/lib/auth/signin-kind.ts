// How this browser's identity was signed, recorded when the sign-in completes: a browser wallet (an extension, or
// WalletConnect) is web3; an in-app (thirdweb) or Magic account is managed. Who sees the Shop's Marketplace follows
// it (ui3 market/access.tsx), never whether a wallet extension happens to be installed in this browser.

export type SignInKind = "web3" | "managed";

const KEY = "dcl:auth:signin-kind:v1";
export const UNRECORDED_BROWSER_WALLET = Symbol("unrecorded browser wallet");

/** Notes how `signer` signed in. */
export function recordSignIn(signer: string, kind: SignInKind, walletRdns: string | null = null): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ signer: signer.toLowerCase(), kind, walletRdns }));
  } catch {
    // Without the record the kind is inferred as before (shop-wallet.ts).
  }
}

function signInRecord(signer: string): { kind: SignInKind; walletRdns?: unknown } | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { signer?: unknown; kind?: unknown; walletRdns?: unknown };
    if (parsed.signer !== signer.toLowerCase()) return null;
    if (parsed.kind !== "web3" && parsed.kind !== "managed") return null;
    return { kind: parsed.kind, walletRdns: parsed.walletRdns };
  } catch {
    return null;
  }
}

/** How `signer` signed in on this browser, when that was recorded. */
export function signInKindFor(signer: string): SignInKind | null {
  return signInRecord(signer)?.kind ?? null;
}

/** Older Web3 sign-ins need to verify the injected account before choosing its provider. */
export function browserWalletRdnsFor(signer: string): string | null | typeof UNRECORDED_BROWSER_WALLET {
  const record = signInRecord(signer);
  if (record?.kind !== "web3") return null;
  if (record.walletRdns === undefined) return UNRECORDED_BROWSER_WALLET;
  return typeof record.walletRdns === "string" && record.walletRdns ? record.walletRdns : null;
}

export function forgetSignIn(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // Nothing to forget.
  }
}
