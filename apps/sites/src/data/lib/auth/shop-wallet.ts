// The Shop's buyer wallet from sites' sign-in, for the web Shop's host (routes/shop/host.tsx) and for the
// in-world Shop's purchases in the web Explorer's shell (routes/routes/play.tsx). How the account signs is
// a fact of the sign-in, never inferred from an address or a balance: an in-app (thirdweb) session for the
// same account is a managed wallet, which signs EIP-712 data through the in-app signer and holds no gas;
// otherwise a browser wallet signs, when one is present.

import type { AuthIdentity } from "@ui/data/auth/identity";
import type { ShellShopWallet } from "@ui/play/shell/shop/purchase";
import type { WalletKind } from "@ui/shop/wallet/session";

import { hasMagicSession } from "./magic-session";
import { signInKindFor, type SignInKind } from "./signin-kind";
import { getThirdwebSession, type ThirdwebSession } from "./thirdweb/session";
import { signTypedData } from "./typed-data";
import { hasWallet, walletProvider, WalletError, type Eip1193Provider } from "./wallet";

/**
 * How the signed identity was made, as the sign-in recorded it (`signin-kind.ts`): a browser wallet or
 * WalletConnect is web3, an in-app (thirdweb) or Magic account managed. An in-app session for the same
 * account is managed whatever the record says. An identity signed before the record existed is managed
 * when a Magic session is held, and otherwise web3 only while a browser wallet is present.
 *
 * With neither here, the web Shop still takes the identity for managed, as it always has: its pages
 * browse, quote and send the buyer to sign in again when the in-app signer has no session. The /play
 * shell (`playShopWallet`) answers none instead, because the in-world Shop offers a purchase only when
 * something on this page can sign it, and says why not otherwise.
 */
export function sitesWalletKind(
  identity: Pick<AuthIdentity, "signer"> | null,
  facts: { inApp: ThirdwebSession | null; injected: boolean; recorded?: SignInKind | null; magic?: boolean },
): WalletKind | null {
  if (!identity) return null;
  if (inAppSessionFor(identity.signer, facts.inApp)) return "managed";
  if (facts.recorded) return facts.recorded;
  if (facts.magic) return "managed";
  return facts.injected ? "web3" : "managed";
}

/** `identity`'s kind from this browser's facts: its sign-in record, its in-app session, a Magic session, a wallet. */
export function currentWalletKind(identity: Pick<AuthIdentity, "signer"> | null): WalletKind | null {
  if (!identity) return null;
  return sitesWalletKind(identity, {
    inApp: getThirdwebSession(),
    injected: hasWallet(),
    recorded: signInKindFor(identity.signer),
    magic: hasMagicSession(),
  });
}

/**
 * Whether `identity`'s kind is known from its sign-in (recorded here, or an in-app session for the account) rather
 * than guessed: a shared identity signed in on another site never was. Only a known web3 sign-in opens the Shop's
 * Marketplace.
 */
export function signInKnown(identity: Pick<AuthIdentity, "signer">): boolean {
  return signInKindFor(identity.signer) !== null || inAppSessionFor(identity.signer);
}

/** Whether this browser's in-app (thirdweb) session is `account`'s. */
export function inAppSessionFor(account: string, session: ThirdwebSession | null = getThirdwebSession()): boolean {
  return session != null && session.address.toLowerCase() === account.toLowerCase();
}

export function browserProvider(): Eip1193Provider | null {
  try {
    return walletProvider();
  } catch {
    return null;
  }
}

type TypedData = Parameters<typeof signTypedData>[0];

/**
 * The managed (in-app) wallet as the Shop's signing seam: it names its account and signs EIP-712 data
 * through the in-app signer, which is all the relayed rails (credits, approvals) ask of it. It holds
 * no gas and sends nothing itself, so every other request is refused; reads go over the Shop's RPC.
 */
export function managedShopProvider(account: string, sign: (typedData: TypedData, from: string) => Promise<string> = signTypedData): Eip1193Provider {
  const self = account.toLowerCase();
  return {
    async request({ method, params }) {
      switch (method) {
        case "eth_accounts":
        case "eth_requestAccounts":
          return [self];
        case "eth_signTypedData_v4": {
          const [from, data] = (params ?? []) as [string, unknown];
          if (typeof from !== "string" || from.toLowerCase() !== self) throw new WalletError("The in-app wallet signs only for its own account.", 4100);
          return sign((typeof data === "string" ? JSON.parse(data) : data) as TypedData, self);
        }
        default:
          throw new WalletError(`The in-app wallet does not support ${method}.`, 4200);
      }
    },
  };
}

/**
 * The in-world Shop's wallet in the web Explorer's shell (/play), for the login the shell holds: managed
 * when this browser's in-app session is that account's, web3 while a browser wallet is present, and none
 * otherwise (an identity whose signer isn't here can't confirm a purchase, so the Shop says so).
 */
export function playShopWallet(): ShellShopWallet {
  const kind = (account: string): WalletKind | null => {
    if (inAppSessionFor(account)) return "managed";
    return hasWallet() ? "web3" : null;
  };
  return {
    kind,
    provider(account) {
      const k = kind(account);
      if (k === "managed") return managedShopProvider(account);
      return k === "web3" ? browserProvider() : null;
    },
  };
}
