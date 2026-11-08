// How this browser's identity was signed, recorded when the sign-in completes: a browser wallet (an extension, or
// WalletConnect) is web3; an in-app (thirdweb) or Magic account is managed. Who sees the Shop's Marketplace follows
// it (ui3 market/access.tsx), never whether a wallet extension happens to be installed in this browser.

export type SignInKind = "web3" | "managed";

const KEY = "dcl:auth:signin-kind:v1";

/** Notes how `signer` signed in. */
export function recordSignIn(signer: string, kind: SignInKind): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ signer: signer.toLowerCase(), kind }));
  } catch {
    // Without the record the kind is inferred as before (shop-wallet.ts).
  }
}

/** How `signer` signed in on this browser, when that was recorded. */
export function signInKindFor(signer: string): SignInKind | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { signer?: unknown; kind?: unknown };
    if (parsed.signer !== signer.toLowerCase()) return null;
    return parsed.kind === "web3" || parsed.kind === "managed" ? parsed.kind : null;
  } catch {
    return null;
  }
}

export function forgetSignIn(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // Nothing to forget.
  }
}
