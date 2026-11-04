// from: auth@e081cc6:src/hooks/useWalletOptions.ts, src/hooks/targetConfig.ts (the default, iOS and Android option sets),
//       src/components/Pages/LoginPage/utils.ts (getSignInOptionsMode, isMobile, isIos),
//       src/components/Connection/Connection.types.ts (connectionOptionTitles)
// decentraland/auth is Apache-2.0.
// Which sign-in options the page offers and where each goes: the email box first, up to two full-width buttons, the
// rest behind "More Options". The `dapps-sign-in-primary-option` rollout picks the layout, read under the same key as
// decentraland.org's sign-in so one switch drives both.

export type ConnectionOption =
  | "metamask"
  | "fortmatic"
  | "coinbase"
  | "wallet-connect"
  | "google"
  | "apple"
  | "discord"
  | "x"
  | "email";

export type SignInOptionsMode = "full" | "one" | "two";

export type OptionSet = { primary: ConnectionOption; secondary?: ConnectionOption; extra: ConnectionOption[] };

export const OPTION_TITLES: Record<ConnectionOption, string> = {
  metamask: "MetaMask",
  fortmatic: "Fortmatic",
  coinbase: "Coinbase",
  "wallet-connect": "WalletConnect",
  google: "Google",
  apple: "Apple",
  discord: "Discord",
  x: "X",
  email: "Email",
};

export const SOCIAL_OPTIONS: readonly ConnectionOption[] = ["google", "apple", "discord", "x"];

const DEFAULT_SET: OptionSet = {
  primary: "email",
  secondary: "metamask",
  extra: ["google", "discord", "apple", "x", "fortmatic", "coinbase", "wallet-connect"],
};
// On a phone the OS decides: no extension wallets, the platform's own sign-in first.
const IOS_SET: OptionSet = { primary: "apple", secondary: "google", extra: ["metamask", "wallet-connect"] };
const ANDROID_SET: OptionSet = { primary: "google", secondary: "apple", extra: ["metamask", "wallet-connect"] };

// iPadOS reports a desktop Safari user agent; touch points give it away.
function isIpadOnDesktopUserAgent(userAgent: string, maxTouchPoints: number): boolean {
  return /Macintosh/i.test(userAgent) && maxTouchPoints > 1;
}

export function optionSetFor(userAgent: string, maxTouchPoints = 0): OptionSet {
  const ipad = isIpadOnDesktopUserAgent(userAgent, maxTouchPoints);
  if (/iPhone|iPad|iPod/i.test(userAgent) || ipad) return IOS_SET;
  if (/Android|webOS|BlackBerry|IEMobile|Opera Mini/i.test(userAgent)) return ANDROID_SET;
  return DEFAULT_SET;
}

/** The rollout's variant: none is the full layout, `two-options` is Google and MetaMask, any other is MetaMask alone. */
export function signInOptionsMode(variant: string | null | undefined): SignInOptionsMode {
  if (!variant) return "full";
  return variant === "two-options" ? "two" : "one";
}

export type Layout = { first?: ConnectionOption; second?: ConnectionOption; more: ConnectionOption[] };

/** Where each option sits. `hasInjectedWallet` is whether the browser carries a wallet (`window.ethereum`). */
export function layoutOptions(set: OptionSet, mode: SignInOptionsMode, hasInjectedWallet: boolean): Layout {
  const primaryIsWallet = set.primary !== "email";
  const all = [...(primaryIsWallet ? [set.primary] : []), ...(set.secondary ? [set.secondary] : []), ...set.extra];

  if (mode === "full") {
    if (primaryIsWallet) return { first: set.primary, second: set.secondary, more: set.extra };
    return { first: set.secondary, second: set.extra[0], more: set.extra.slice(1) };
  }

  const google = all.find((o) => o === "google");
  const metamask = all.find((o) => o === "metamask");
  if (mode === "two") {
    if (hasInjectedWallet && metamask) {
      return { first: google, second: metamask, more: all.filter((o) => o !== google && o !== metamask) };
    }
    return { first: google, more: all.filter((o) => o !== google) };
  }

  if (hasInjectedWallet && metamask) return { first: metamask, more: all.filter((o) => o !== metamask) };
  const rest = all.filter((o) => o !== google && o !== metamask);
  return { more: [google, metamask, ...rest].filter((o): o is ConnectionOption => o !== undefined) };
}
