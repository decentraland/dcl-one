import { startWalletConnectSignIn } from "@ui/data/auth/walletconnect";
import { setIdentity } from "./session";
export { hasWalletConnect } from "@ui/data/auth/walletconnect";
export type { WalletConnectSession } from "@ui/data/auth/walletconnect";

export async function connectWalletConnect(opts: { signal?: AbortSignal; expirationMs?: number } = {}) {
  const session = await startWalletConnectSignIn(opts);
  return {
    ...session,
    connected: session.connected.then((identity) => {
      if (opts.signal?.aborted) return null;
      if (identity) setIdentity(identity);
      return identity;
    }),
  };
}
