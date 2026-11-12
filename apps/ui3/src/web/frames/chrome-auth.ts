import { createContext, useContext } from "react";

export type ChromeAuth = {
  signedIn: boolean;
  account: string;
  mana: string;
  name: string;
  committee: boolean;
  avatarUrl?: string;
  /** The avatar disc's background once the profile loads (web/places/avatarColor.ts); the CSS default before. */
  avatarColor?: string;
  onSignIn?: () => void;
  onSignOut?: () => void;
  onSwitchAccount?: () => void;
  fetchNotifications?: (signal?: AbortSignal) => Promise<unknown>;
};

const LOGGED_OUT: ChromeAuth = {
  signedIn: false,
  account: "",
  mana: "",
  name: "",
  committee: false,
  avatarUrl: undefined,
  avatarColor: undefined,
  onSignIn: undefined,
  onSignOut: undefined,
  onSwitchAccount: undefined,
  fetchNotifications: undefined,
};

export const ChromeAuthContext = createContext<ChromeAuth>(LOGGED_OUT);

export function useChromeAuth(): ChromeAuth {
  return useContext(ChromeAuthContext);
}
