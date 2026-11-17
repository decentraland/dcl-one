import type { ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button, Spinner } from "@dcl/one-ui";
import { GeneralClose, GeneralLeftArrow } from "@dcl/one-ui/foundations/icons";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { openSignIn } from "../components/auth/signin-store";

import styles from "./AccountLayout.module.css";
import { AccountSidebar, type AccountSection } from "./AccountSidebar";
import { ACCOUNT_COPY } from "./copy";
import { useAccountIdentity } from "./useAccountIdentity";

export type { AccountSection } from "./AccountSidebar";

type AccountLayoutProps = {
  /** The open section; null on `/account` itself, which a phone shows as the section menu. */
  active: AccountSection | null;
  /** Rendered once the browser holds a session, keyed by its account so a switch starts afresh. */
  children: (identity: AuthIdentity) => ReactNode;
};

/** The browser router's position in this tab's history; 0 when there is none to read. */
function historyIndex(): number {
  const state: unknown = typeof window === "undefined" ? null : window.history.state;
  const index = state && typeof state === "object" && "idx" in state ? Number(state.idx) : 0;
  return Number.isFinite(index) ? index : 0;
}

/** The history position the reader entered the settings at, carried by every link inside them. */
export type AccountLinkState = { accountEntry: number };

function entryOf(state: unknown): number | null {
  if (!state || typeof state !== "object" || !("accountEntry" in state)) return null;
  const entry = Number(state.accountEntry);
  return Number.isInteger(entry) && entry >= 0 ? entry : null;
}

/**
 * Where the settings were entered, and Close: back to the page before that entry, past every menu and
 * section screen visited since, or home when the settings were the first page in the tab.
 */
function useAccountEntry(): { linkState: AccountLinkState; close: () => void } {
  const navigate = useNavigate();
  const { state } = useLocation();
  const carried = entryOf(state);
  const entry = carried ?? historyIndex();
  return {
    linkState: { accountEntry: entry },
    close: () => {
      if (entry > 0) void navigate(entry - 1 - historyIndex());
      else void navigate("/");
    },
  };
}

/**
 * The account settings frame. From 1024px the section menu is a left rail beside the open section.
 * Below it the settings are master-detail: `/account` is the menu on its own, and each section is its
 * own screen under a "‹ Section ✕" header that goes back to the menu.
 */
export function AccountLayout({ active, children }: AccountLayoutProps) {
  const account = useAccountIdentity();
  const { linkState, close } = useAccountEntry();

  return (
    <SitesChrome active="play" overlayNav>
      <div className={styles.root} data-view={account.state === "signed-in" && !active ? "menu" : "section"}>
        {account.state === "signed-in" ? (
          <div className={styles.frame}>
            <AccountSidebar
              className={styles.sidebar}
              address={account.identity.signer}
              active={active}
              linkState={linkState}
            />
            <div className={styles.content}>
              {active ? (
                <div className={styles.mobileHeader}>
                  <h1 className={styles.mobileTitle}>
                    <Link
                      to="/account"
                      state={linkState}
                      className={styles.back}
                      aria-label={`${ACCOUNT_COPY.sections[active]}, ${ACCOUNT_COPY.back}`}
                    >
                      <GeneralLeftArrow size={16} aria-hidden="true" />
                      {ACCOUNT_COPY.sections[active]}
                    </Link>
                  </h1>
                  <button type="button" className={styles.close} aria-label={ACCOUNT_COPY.close} onClick={close}>
                    <GeneralClose size={20} aria-hidden="true" />
                  </button>
                </div>
              ) : null}
              <div key={account.identity.signer.toLowerCase()}>{children(account.identity)}</div>
            </div>
          </div>
        ) : account.state === "signed-out" ? (
          <div className={styles.signedOut}>
            <h1 className={styles.signedOutTitle}>{ACCOUNT_COPY.signedOut}</h1>
            <Button size="md" onClick={() => openSignIn()}>
              {ACCOUNT_COPY.signIn}
            </Button>
          </div>
        ) : (
          <div className={styles.checking} role="status">
            <Spinner variant="light" aria-hidden="true" />
            <span>{ACCOUNT_COPY.checking}</span>
          </div>
        )}
      </div>
    </SitesChrome>
  );
}
