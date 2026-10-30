// from: shop@94584a5c:app/src/hooks/useAccountWatcher.ts, app/src/hooks/useWalletChain.ts, app/src/store/wallet.ts
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { sessionAccount, SIGNED_OUT, type Session } from "../model/settlement";
import { shopCart, type CartStore } from "../cart/store";
import { isUserRejection, readChainId, requestChainSwitch, subscribeWallet } from "./chain";
import { applyWalletReturn } from "./returnTo";
import { shopSession, type ShopWalletHost } from "./session";

export type WalletChain = {
  chainId: number | null;
  pendingChainId: number | null;
  switchTo(chainId: number): Promise<void>;
};

export function useWalletChain(provider: Eip1193Provider | null): WalletChain {
  const [chainId, setChainId] = useState<number | null>(null);
  const [pendingChainId, setPending] = useState<number | null>(null);
  const pending = useRef<number | null>(null);

  useEffect(() => {
    if (!provider) {
      setChainId(null);
      return;
    }
    let live = true;
    const refresh = () => {
      readChainId(provider).then(
        (next) => live && setChainId(next),
        () => {
        },
      );
    };
    refresh();
    const off = subscribeWallet(provider, {
      onChainChanged: () => {
        pending.current = null;
        setPending(null);
        refresh();
      },
      onAccountsChanged: refresh,
    });
    return () => {
      live = false;
      off();
    };
  }, [provider]);

  const switchTo = useCallback(
    async (target: number) => {
      if (!provider || target === chainId || pending.current !== null) return;
      pending.current = target;
      setPending(target);
      try {
        await requestChainSwitch(provider, target);
        setChainId(await readChainId(provider));
      } catch (e) {
        if (!isUserRejection(e)) console.warn("[shop/wallet] network switch failed", e);
      } finally {
        pending.current = null;
        setPending(null);
      }
    },
    [provider, chainId],
  );

  return { chainId, pendingChainId, switchTo };
}

export function useWalletAccount(provider: Eip1193Provider | null): string | null {
  const [account, setAccount] = useState<string | null>(null);
  useEffect(() => {
    if (!provider) {
      setAccount(null);
      return;
    }
    let live = true;
    provider.request({ method: "eth_accounts" }).then(
      (list) => live && setAccount((list as string[] | undefined)?.[0]?.toLowerCase() ?? null),
      () => {},
    );
    const off = subscribeWallet(provider, { onAccountsChanged: (next) => setAccount(next) });
    return () => {
      live = false;
      off();
    };
  }, [provider]);
  return account;
}

export type ShopWalletState = WalletChain & {
  host: ShopWalletHost | null;
  session: Session;
  account: string | null;
  walletAccount: string | null;
  accountMismatch: boolean;
  resolving: boolean;
  revision: string;
};

const SIGNED_OUT_STATE: ShopWalletState = {
  host: null,
  session: SIGNED_OUT,
  account: null,
  walletAccount: null,
  accountMismatch: false,
  resolving: false,
  chainId: null,
  pendingChainId: null,
  switchTo: async () => {},
  revision: "out",
};

const ShopWalletContext = createContext<ShopWalletState>(SIGNED_OUT_STATE);

export function ShopWalletProvider({
  host,
  cart = shopCart,
  children,
}: {
  host: ShopWalletHost;
  cart?: CartStore;
  children: ReactNode;
}) {
  const session = useMemo(() => shopSession(host), [host.identity, host.walletKind]);
  const account = sessionAccount(session);
  const provider = useMemo(
    () => (session.kind === "web3" ? (host.provider?.() ?? null) : null),
    [session.kind, host.provider],
  );
  const chain = useWalletChain(provider);
  const walletAccount = useWalletAccount(provider);

  useEffect(() => {
    cart.scopeTo(account);
    applyWalletReturn(account, cart);
  }, [cart, account, session.kind]);

  const accountMismatch = !!walletAccount && !!account && walletAccount !== account;
  const resolving = !!host.resolving;
  const value = useMemo<ShopWalletState>(
    () => ({
      ...chain,
      host,
      session,
      account,
      walletAccount,
      accountMismatch,
      resolving,
      revision: `${session.kind}:${account ?? "-"}:${walletAccount ?? "-"}:${chain.chainId ?? "-"}`,
    }),
    [chain, host, session, account, walletAccount, accountMismatch, resolving],
  );
  return <ShopWalletContext.Provider value={value}>{children}</ShopWalletContext.Provider>;
}

export function useShopWallet(): ShopWalletState {
  return useContext(ShopWalletContext);
}
