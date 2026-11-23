import { useEffect, useState } from "react";

import type { Eip1193Provider } from "../../../web-core/auth/eip1193";
import { subscribeWallet } from "../../../web-core/auth/wallet-events";

export function useWalletAccount(provider: Eip1193Provider | null): string | null {
  const [account, setAccount] = useState<string | null>(null);
  useEffect(() => {
    if (!provider) {
      setAccount(null);
      return;
    }
    let live = true;
    let receivedAccountChange = false;
    const off = subscribeWallet(provider, {
      onAccountsChanged: (next) => {
        receivedAccountChange = true;
        setAccount(next);
      },
    });
    provider.request({ method: "eth_accounts" }).then(
      (list) => {
        if (live && !receivedAccountChange) {
          setAccount((list as string[] | undefined)?.[0]?.toLowerCase() ?? null);
        }
      },
      () => {},
    );
    return () => {
      live = false;
      off();
    };
  }, [provider]);
  return account;
}
