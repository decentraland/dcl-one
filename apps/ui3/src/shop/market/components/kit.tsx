// The small pieces every seller and marketplace page shares, on ui3's atoms and the Shop's wallet seam:
// a query client when the host has none, the connected wallet's EIP-1193 provider, the sign-in gate,
// exact MANA amounts, the outage explanation for a route whose index has not caught up, the wrong-network
// notice (switching only on the owner's click), and the progress of a wallet write.

import { QueryClient, QueryClientContext, QueryClientProvider } from "@tanstack/react-query";
import { useContext, useMemo, useState, type ReactNode } from "react";

import Button from "../../../atoms/Button";
import ManaMark from "../../../atoms/ManaMark";
import Spinner from "../../../atoms/Spinner";
import EmptyState from "../../../components/EmptyState";
import type { Eip1193Provider } from "../../../data/auth/wallet";
import { chainLabel, isPolygon } from "../../model/chain";
import { formatMana, type ManaWei } from "../../model/money";
import { useShopWallet } from "../../wallet/context";
import { isWrongNetworkError } from "../../wallet/chain";
import { dataUnavailable } from "../lib/api";
import { declined, isTxPending, isTxReverted } from "../lib/tx";
import "../market.css";

let fallbackClient: QueryClient | null = null;

/** Pages use react-query; a host without a QueryClientProvider (a bare Explorer mount, a test) gets one. */
export function EnsureQueryClient({ children }: { children: ReactNode }) {
  const existing = useContext(QueryClientContext);
  if (existing) return <>{children}</>;
  fallbackClient ??= new QueryClient({ defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false, staleTime: 30_000 } } });
  return <QueryClientProvider client={fallbackClient}>{children}</QueryClientProvider>;
}

/** The connected web3 wallet's provider; null for a web2 session (it has no wallet to sign with). */
export function useShopProvider(): Eip1193Provider | null {
  const wallet = useShopWallet();
  return useMemo(() => (wallet.session.kind === "web3" ? (wallet.host?.provider?.() ?? null) : null), [wallet.session.kind, wallet.host]);
}

export function ShopPage({ title, actions, children, className }: { title: ReactNode; actions?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <EnsureQueryClient>
      <section className={`mk-page${className ? ` ${className}` : ""}`}>
        <header className="mk-page__head">
          <h1 className="mk-page__title">{title}</h1>
          {actions ? <div className="mk-page__actions">{actions}</div> : null}
        </header>
        {children}
      </section>
    </EnsureQueryClient>
  );
}

export function SignInGate({ title, body }: { title: string; body: string }) {
  const { host } = useShopWallet();
  return (
    <EmptyState
      variant="screen"
      title={title}
      subtitle={body}
      actions={host?.onSignIn ? [{ label: "Sign in", onClick: () => host.onSignIn?.() }] : []}
      data-testid="shop-signin-gate"
    />
  );
}

/** Seller writes need a wallet that can sign: a web2 session is told how to connect one (never offered credits). */
export function WalletRequired({ what }: { what: string }) {
  const { host, account } = useShopWallet();
  return (
    <div className="mk-note" role="status" data-testid="shop-wallet-required">
      <p>
        {what} needs a web3 wallet to sign{account ? ` for ${account.slice(0, 6)}…${account.slice(-4)}` : ""}.
      </p>
      {host?.onConnectWallet ? (
        <Button size="sm" onClick={() => host.onConnectWallet?.()}>
          Connect a wallet
        </Button>
      ) : null}
    </div>
  );
}

/**
 * A write signs with the wallet's CURRENT account, so it only proceeds when that is the signed-in account:
 * a web2 session is told to connect a wallet, and a wallet moved to another account is an explicit
 * switch (sign in again as it, or move the wallet back), never a silent signature for someone else.
 */
export function WalletGate({ what, children }: { what: string; children: ReactNode }) {
  const block = useWalletBlock(what);
  return <>{block ?? children}</>;
}

/** What stands in for a write's form when it cannot sign for the signed-in account; null when it can. */
export function useWalletBlock(what: string): ReactNode | null {
  const wallet = useShopWallet();
  const provider = useShopProvider();
  if (wallet.session.kind !== "web3" || !provider) return <WalletRequired what={what} />;
  if (wallet.accountMismatch && wallet.walletAccount && wallet.account)
    return (
      <div className="mk-note mk-note--warning" role="alert" data-testid="shop-account-mismatch">
        <p>
          Your wallet is on {shortAddr(wallet.walletAccount)}, but you are signed in as {shortAddr(wallet.account)}. {what} signs with your wallet, so switch it back to{" "}
          {shortAddr(wallet.account)}, or sign in as {shortAddr(wallet.walletAccount)}.
        </p>
        {wallet.host?.onSwitchAccount ? (
          <Button size="sm" variant="secondary" onClick={() => wallet.host?.onSwitchAccount?.()}>
            Sign in as {shortAddr(wallet.walletAccount)}
          </Button>
        ) : null}
      </div>
    );
  return null;
}

export function ManaAmount({ wei, network = "polygon", className, decimals }: { wei: bigint; network?: "polygon" | "ethereum"; className?: string; decimals?: number }) {
  return (
    <span className={`mk-mana${className ? ` ${className}` : ""}`}>
      <ManaMark size={14} network={network} />
      <span>{formatMana(wei as ManaWei, { group: true, decimals })}</span>
      <span className="u-sr-only"> MANA</span>
    </span>
  );
}

export function manaNetworkOf(chainId: number): "polygon" | "ethereum" {
  return isPolygon(chainId) ? "polygon" : "ethereum";
}

const SOURCE_TEXT: Record<string, string> = {
  "nft-indexer": "the NFT index",
  "trade-events": "the trade events index",
  coupons: "the coupons index",
  "catalyrst-fed": "federation",
};

/** A read failed: a 503 unavailable envelope names what is catching up; anything else is a plain error with retry. */
export function LoadFailure({ error, what, onRetry }: { error: unknown; what: string; onRetry?: () => void }) {
  const outage = dataUnavailable(error);
  return (
    <div className="mk-note mk-note--error" role="alert" data-testid={outage ? "shop-outage" : "shop-error"}>
      {outage ? (
        <p>
          {what} is not available on this node right now: {SOURCE_TEXT[outage.source] ?? outage.source} is still catching up. Nothing is
          missing from your account; try again later.
        </p>
      ) : (
        <p>We could not load {what.toLowerCase()}.</p>
      )}
      {onRetry ? (
        <Button size="sm" variant="secondary" onClick={onRetry}>
          Try again
        </Button>
      ) : null}
    </div>
  );
}

export function Loading({ label }: { label: string }) {
  return (
    <div className="mk-loading" role="status" aria-busy="true">
      <Spinner size={24} aria-hidden />
      <span>{label}</span>
    </div>
  );
}

/** The wallet is on another chain than the action needs: say so, and switch only from this click. */
export function NetworkNotice({ chainId }: { chainId: number }) {
  const wallet = useShopWallet();
  if (wallet.chainId === null || wallet.chainId === chainId) return null;
  return (
    <div className="mk-note" role="status" data-testid="shop-network-notice">
      <p>
        Your wallet is on {chainLabel(wallet.chainId)}. This runs on {chainLabel(chainId)}.
      </p>
      <Button size="sm" variant="secondary" disabled={wallet.pendingChainId != null} onClick={() => void wallet.switchTo(chainId)}>
        {wallet.pendingChainId != null ? "Confirm in your wallet…" : `Switch to ${chainLabel(chainId)}`}
      </Button>
    </div>
  );
}

export type WriteState = { kind: "idle" } | { kind: "working"; label: string } | { kind: "done"; hash: string | null; label: string } | { kind: "failed"; error: unknown };

/** What a failed write means to its owner: declined, still pending, reverted, wrong network, or an error. */
export function writeFailureText(error: unknown): string {
  if (declined(error)) return "You declined in your wallet. Nothing was sent.";
  if (isTxPending(error)) return "The transaction is still pending. It may still complete; check again before trying anything else.";
  if (isTxReverted(error)) return "The transaction was reverted on chain. Nothing changed.";
  if (isWrongNetworkError(error)) return (error as Error).message;
  return (error as Error)?.message || "Something went wrong.";
}

export function WriteStatus({ state }: { state: WriteState }) {
  if (state.kind === "idle") return null;
  if (state.kind === "working")
    return (
      <p className="mk-write mk-write--working" role="status" aria-live="polite">
        <Spinner size={16} aria-hidden /> {state.label}
      </p>
    );
  if (state.kind === "done")
    return (
      <p className="mk-write mk-write--done" role="status">
        {state.label}
      </p>
    );
  return (
    <p className="mk-write mk-write--failed" role="alert">
      {writeFailureText(state.error)}
    </p>
  );
}

/** A write's lifecycle as one state value, with the owner's click as the only trigger. */
export function useWrite() {
  const [state, setState] = useState<WriteState>({ kind: "idle" });
  const run = async (label: string, doneLabel: string, action: (progress: (label: string) => void) => Promise<string | null | void>) => {
    setState({ kind: "working", label });
    try {
      const hash = await action((next) => setState({ kind: "working", label: next }));
      setState({ kind: "done", hash: hash ?? null, label: doneLabel });
      return true;
    } catch (error) {
      setState({ kind: "failed", error });
      return false;
    }
  };
  return { state, run, reset: () => setState({ kind: "idle" }), busy: state.kind === "working" };
}

export function formatDate(ms: number): string {
  return new Date(ms).toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
}

export function shortAddr(address: string): string {
  return address.length > 12 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address;
}
