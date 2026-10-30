import type { AuthIdentity } from "../../data/auth/identity";
import type { Eip1193Provider } from "../../data/auth/wallet";
import { parseEvmAddress } from "../model/address";
import { SIGNED_OUT, type Session } from "../model/settlement";

export type WalletKind = "web3" | "managed";

export type ShopWalletHost = {
  identity: AuthIdentity | null;
  walletKind: WalletKind | null;
  provider?: (() => Eip1193Provider | null) | null;
  onSignIn?(): void;
  onConnectWallet?(): void;
  onSwitchAccount?(): void;
  resolving?: boolean;
};

export function isExpiredIdentity(identity: AuthIdentity, now: number = Date.now()): boolean {
  const at = Date.parse(identity.expiration);
  return !Number.isFinite(at) || at <= now;
}

export function shopSession(host: Pick<ShopWalletHost, "identity" | "walletKind">, now: number = Date.now()): Session {
  const identity = host.identity;
  if (!identity || isExpiredIdentity(identity, now)) return SIGNED_OUT;
  const address = parseEvmAddress(identity.signer);
  if (!address) return SIGNED_OUT;
  return host.walletKind === "web3" ? { kind: "web3", wallet: address } : { kind: "web2", account: address };
}
