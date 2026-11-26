// The context every purchase rail runs in: who is buying, the wallet that signs (ui3's EIP-1193 seam),
// the read-only chain access, the one Shop client, and how the relay is reached. Upstream threaded an
// ethers Signer and read the session from a zustand store; here it is one explicit value, so a rail
// can run in the Explorer, in sites, or against fakes in a test with the same code.

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ShopClient } from "@shop/api/shop";
import type { WalletKind } from "../../wallet/session";
import type { ChainReader } from "../chain/rpc";

export type RailContext = {
  /** Lowercase buyer address: the signed session's account, which is also the beneficiary. */
  buyer: string;
  provider: Eip1193Provider;
  reader: ChainReader;
  client: ShopClient;
  walletKind: WalletKind | null;
  relayerUrl?: string;
  fetchImpl?: typeof fetch;
  /** Relayed rails on (the default) or off. */
  gasless?: boolean;
  /**
   * A MANA sale only through the relay, never sent from the wallet instead: the in-world Shop's, so a sale whose
   * answer is lost can be retried with the same meta-transaction nonce and land at most once.
   */
  relayOnly?: boolean;
  /** The browser's Web Locks an exact MANA grant runs under (the page's by default; null: none). */
  locks?: LockManager | null;
  /** The caller already holds the exact grant's lock (`exactGrantLock`): a credits + MANA sale takes it before its item's. */
  grantLockHeld?: boolean;
  /**
   * Whether the purchase this run serves is still its caller's (its dialog open, its terms the ones agreed): an exact MANA
   * grant whose purchase is gone while the wallet signs records and sends nothing (`GrantCancelledError`).
   */
  live?: () => boolean;
  /** Receipt/nonce wait tuning, for tests. */
  wait?: { timeoutMs?: number; pollMs?: number; now?: () => number; sleep?: (ms: number) => Promise<void> };
};
