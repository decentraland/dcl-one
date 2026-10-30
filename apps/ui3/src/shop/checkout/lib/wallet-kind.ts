// from: shop@94584a5c:app/src/lib/wallet-kind.ts, app/src/lib/wallet.ts
// Wallet UX classification on the host's declared wallet kind (wallet/session.ts `WalletKind`) instead
// of @dcl/schemas' ProviderType: only a self-custody web3 wallet shows a prompt per on-chain action and
// holds gas. Anything else (an embedded/email wallet, or an unknown kind) is managed, so it never sees
// approval, gas or network wording. An allowlist: an unknown kind cannot leak MetaMask-style copy.

import type { WalletKind } from "../../wallet/session";

/** True when the connected wallet pops a confirmation for each on-chain action. */
export function showsWalletConfirmations(kind?: WalletKind | null): boolean {
  return kind === "web3";
}

/** May this wallet be offered a rail where the wallet itself pays gas? */
export function canPayGasItself(kind?: WalletKind | null): boolean {
  return showsWalletConfirmations(kind);
}

/** A managed (web2) wallet: signs transparently, gasless, single step. */
export function isManagedWallet(session: { walletKind?: WalletKind | null } | null | undefined): boolean {
  return !!session && !showsWalletConfirmations(session.walletKind);
}
