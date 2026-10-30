// The wallet as the LAND and NAME flows need it: the connected EIP-1193 provider of a web3 session,
// the account it signs for, and whether it is on the chain a step needs. Built on useShopWallet; a
// wallet on another account than the signed session is never used to sign (`accountMismatch`).

import { useMemo } from "react";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { useShopWallet } from "../wallet/context";

export type LandWallet = ReturnType<typeof useShopWallet> & {
  provider: Eip1193Provider | null;
  /** The account that signs: the session's, when the wallet agrees with it. */
  signer: string | null;
  onChain(chainId: number): boolean;
};

export function useLandWallet(): LandWallet {
  const wallet = useShopWallet();
  const provider = useMemo(() => (wallet.session.kind === "web3" ? (wallet.host?.provider?.() ?? null) : null), [wallet.session.kind, wallet.host]);
  const signer = wallet.session.kind === "web3" && !wallet.accountMismatch ? wallet.account : null;
  return { ...wallet, provider, signer, onChain: (chainId: number) => wallet.chainId === chainId };
}
