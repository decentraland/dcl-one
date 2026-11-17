import type { ReactNode } from "react";
import { Link } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button, Spinner, TabsBar, type TabsBarLinkProps } from "@dcl/one-ui";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { openSignIn } from "@/features/components/auth/signin-store";

import styles from "./AccountLayout.module.css";
import { ACCOUNT_COPY } from "./copy";
import { useAccountIdentity } from "./useAccountIdentity";

export type AccountSection = "notifications" | "credits";

/** Wallets, security and account deletion wait on scope decisions, so they are not listed. */
const SECTIONS = [
  { id: "notifications", label: ACCOUNT_COPY.sections.notifications, href: "/account/notifications" },
  { id: "credits", label: ACCOUNT_COPY.sections.credits, href: "/account/credits" },
] as const;

function SectionLink({ href, prefetch, ...props }: TabsBarLinkProps) {
  return <Link to={href} prefetch={prefetch} {...props} />;
}

type AccountLayoutProps = {
  active: AccountSection | null;
  /** Rendered once the browser holds a session, keyed by its account so a switch starts afresh. */
  children: (identity: AuthIdentity) => ReactNode;
};

/** The account settings frame: Sites chrome, title, section navigation, and the signed-in section. */
export function AccountLayout({ active, children }: AccountLayoutProps) {
  const account = useAccountIdentity();

  return (
    <SitesChrome active="play">
      <div className={styles.root}>
        <div className={styles.content}>
          <h1 className={styles.title}>{ACCOUNT_COPY.title}</h1>
          <TabsBar
            mode="navigation"
            label={ACCOUNT_COPY.title}
            items={SECTIONS}
            activeId={active ?? ""}
            LinkComponent={SectionLink}
            className={styles.sections}
          />
          <div className={styles.section}>
            {account.state === "signed-in" ? (
              <div key={account.identity.signer.toLowerCase()}>{children(account.identity)}</div>
            ) : account.state === "signed-out" ? (
              <div className={styles.signedOut}>
                <p>{ACCOUNT_COPY.signedOut}</p>
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
        </div>
      </div>
    </SitesChrome>
  );
}
