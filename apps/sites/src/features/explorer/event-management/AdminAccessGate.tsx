import { adminCopy } from "@dcl/one-ui/apps/sites/events/admin-copy";
import { adminPageStyles } from "@dcl/one-ui/apps/sites/events/AdminPage";
import { Busy } from "@dcl/one-ui/apps/sites/events/Busy";
import { Notice } from "@dcl/one-ui/apps/sites/events/Notice";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import type { ReactNode } from "react";
import { Navigate } from "react-router";

import { openSignIn } from "../../components/auth/signin-store";
import { useSignInRequired } from "../../components/auth/useSignInRequired";
import type { EventAdminAccess } from "./useEventAdmin";

/**
 * The admin pages' entry: a signed-out visitor is sent to sign in, an account without the page's right
 * goes back to /events (as decentraland.org's `<Navigate to="/events" replace />`), and a failed
 * permission read offers a retry instead of a redirect.
 */
export function AdminAccessGate({ access, allowed, children }: { access: EventAdminAccess; allowed: boolean; children: ReactNode }) {
  useSignInRequired(true, Boolean(access.identity));
  if (!access.identity) {
    return (
      <div className={adminPageStyles.status}>
        <p>Sign in to continue.</p>
        <Button onClick={() => openSignIn()}>Sign in</Button>
      </div>
    );
  }
  if (access.loading) return <Busy layout="page">{adminCopy.pending.loading}</Busy>;
  if (access.error) {
    return (
      <div className={adminPageStyles.status}>
        <Notice tone="error">{access.error.message}</Notice>
        <Button variant="secondary" onClick={access.retry}>{adminCopy.pending.retry}</Button>
      </div>
    );
  }
  if (!allowed) return <Navigate to="/events" replace />;
  return <>{children}</>;
}
