// from: auth@e081cc6:src/components/Pages/LoginPage/LoginPage.tsx (handleOnConnect: the connect phase, then the
//       signature, and which modal state each failure shows), src/components/ConnectionModal/ConnectionLayout.type.ts
// decentraland/auth is Apache-2.0.
// A browser wallet's sign-in in two phases, so a failure is described by what the reader was actually asked: a wallet
// that never showed a prompt did not "fail to confirm" anything.

import { createIdentityFor } from "@/data/lib/auth/identity";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { connectWallet, detectWallets, getConnectedAddress, selectWallet } from "@/data/lib/auth/wallet";

export type ConnectionState =
  | "connecting"
  | "signing"
  | "redirecting"
  | "error"
  | "error-generic"
  | "error-locked"
  | "error-signature-unsupported"
  | "error-connection-failed";

export const LOADING_STATES: readonly ConnectionState[] = ["connecting", "signing", "redirecting"];

type WalletFailure = { code?: unknown; message?: unknown; name?: unknown };

function failure(err: unknown): WalletFailure {
  return typeof err === "object" && err !== null ? (err as WalletFailure) : {};
}

/** The reader turned the request down in their wallet (EIP-1193 4001, or a wallet that only says so in words). */
export function isRejection(err: unknown): boolean {
  const f = failure(err);
  if (f.code === 4001 || f.code === "ACTION_REJECTED") return true;
  return typeof f.message === "string" && /user (rejected|denied)|rejected the request/i.test(f.message);
}

/** Conditions already understood, whose technical text adds nothing: a rejection, or a prompt already open. */
export function isExpectedWalletError(err: unknown): boolean {
  return isRejection(err) || failure(err).code === -32002;
}

function isSignatureUnsupported(err: unknown): boolean {
  const code = failure(err).code;
  return code === 4200 || code === -32601;
}

function message(err: unknown): string | null {
  const m = failure(err).message;
  return typeof m === "string" && m ? m : null;
}

/** The modal state, and the detail it quotes, for a failure; `askedToSign` is whether the signature was requested. */
export function failureState(err: unknown, askedToSign: boolean): { state: ConnectionState; detail: string | null } {
  if (askedToSign && isSignatureUnsupported(err)) return { state: "error-signature-unsupported", detail: null };
  if (failure(err).name === "ErrorUnlockingWallet") return { state: "error-locked", detail: null };
  if (!askedToSign) return { state: "error-connection-failed", detail: isExpectedWalletError(err) ? null : message(err) };
  if (isExpectedWalletError(err)) return { state: "error", detail: null };
  return { state: "error-generic", detail: message(err) };
}

/** The announced wallet to use for an option: MetaMask's or Coinbase's own provider when the browser lists it. */
export function walletFor(option: "metamask" | "coinbase"): string | null {
  const rdns = option === "metamask" ? "io.metamask" : "com.coinbase.wallet";
  return detectWallets().some((w) => w.rdns === rdns) ? rdns : null;
}

/** Whether the browser carries MetaMask, the check upstream uses to enable its button. */
export function hasMetaMask(): boolean {
  if (typeof window === "undefined") return false;
  const injected = window.ethereum as (typeof window.ethereum & { isMetaMask?: boolean }) | undefined;
  return !!injected?.isMetaMask || walletFor("metamask") !== null;
}

/**
 * Connects the browser wallet, then asks it to sign a session; `onPhase` hears each step as it starts. The session is
 * returned, not kept: the caller keeps it only if the reader is still waiting for this attempt.
 */
export async function signInWithBrowserWallet(
  rdns: string | null,
  onPhase: (phase: "connecting" | "signing") => void,
): Promise<AuthIdentity> {
  selectWallet(rdns);
  onPhase("connecting");
  const signer = (await getConnectedAddress()) ?? (await connectWallet());
  onPhase("signing");
  return createIdentityFor(signer);
}
