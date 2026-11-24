import {
  adminCopy,
  AdminPage,
  adminPageStyles,
  Notice,
  PermissionsModal,
  UsersTable,
  type AdminUserRow,
} from "@dcl/one-ui";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { getIdentity } from "@/data/lib/auth/session";
import { eventAdminKeys, loadEventProfileSettings, saveEventPermissions, type EventProfile } from "@/data/lib/catalyst/places/event-management";
import { creatorProfilesQuery } from "@/data/lib/catalyst/social/creator-profiles";
import { useEventsFooterCopy } from "../../events/locale";
import { openSignIn } from "../../components/auth/signin-store";
import SitesLayout from "../../components/chrome/SitesLayout";
import { AdminAccessGate } from "./AdminAccessGate";
import { EventAdminTabs } from "./EventAdminTabs";
import { useEventAdmin, type EventAdminAccess } from "./useEventAdmin";
import { useToast } from "./useToast";

const SELF_LOCKED = "edit_any_profile";

type Editing = { user?: AdminUserRow; permissions: readonly string[] };

/** The wallet's current grants with the switches that changed from `initial` to `chosen` applied. */
function applyPermissionChanges(current: readonly string[], initial: readonly string[], chosen: readonly string[]): string[] {
  const removed = new Set(initial.filter((permission) => !chosen.includes(permission)));
  return [...new Set([...current.filter((permission) => !removed.has(permission)), ...chosen.filter((permission) => !initial.includes(permission))])];
}

export default function UsersAdminPage() {
  const access = useEventAdmin();
  const footerCopy = useEventsFooterCopy();
  return (
    <SitesLayout active="whatson" headerSurface="overlay" overlayNav footerCopy={footerCopy}>
      <AdminPage bar={<EventAdminTabs active="users" />}>
        <AdminAccessGate access={access} allowed={access.canManageUsers}>
          <UsersAdmin key={access.identity?.signer.toLowerCase()} access={access} profiles={access.profiles ?? []} />
        </AdminAccessGate>
      </AdminPage>
    </SitesLayout>
  );
}

function UsersAdmin({ access, profiles }: { access: EventAdminAccess; profiles: EventProfile[] }) {
  const client = useQueryClient();
  const names = useQuery({ ...creatorProfilesQuery(profiles.map((profile) => profile.user)), enabled: profiles.length > 0 });
  const [editing, setEditing] = useState<Editing | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useToast();
  const mounted = useRef(true);
  useEffect(() => {
    // StrictMode runs this setup again after a cleanup, so the flag is restored here, not only at creation.
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const rows: AdminUserRow[] = profiles.map((profile) => {
    const found = names.data?.[profile.user.toLowerCase()];
    return { address: profile.user, name: found?.name || undefined, avatarUrl: found?.avatarUrl, permissions: profile.permissions };
  });

  const listed = (address: string) => profiles.find((profile) => profile.user.toLowerCase() === address.toLowerCase());

  function open(next: Editing) {
    setError(null);
    setEditing(next);
  }

  async function save({ address, permissions, initial }: { address: string; permissions: string[]; initial: string[] }) {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== access.identity?.signer.toLowerCase()) {
      openSignIn();
      return;
    }
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      // A save replaces the wallet's whole set, and the switches started from a list read up to the access
      // query's stale time ago (or from nothing, for an address typed in Add). So the wallet's grants are read
      // now and only the switches that changed are applied to them: a grant another moderator added since
      // the list loaded is kept.
      let held: EventProfile | null;
      try {
        held = await loadEventProfileSettings(signer, address);
      } catch {
        if (mounted.current) setError(adminCopy.permissions.readError);
        return;
      }
      const changed = applyPermissionChanges(held?.permissions ?? [], initial, permissions);
      // The signed-in account keeps its own user administration, as the dialog's locked switch shows.
      const own = address.toLowerCase() === signer.signer.toLowerCase();
      const sent = own && !changed.includes(SELF_LOCKED) ? [...changed, SELF_LOCKED] : changed;
      const saved = await saveEventPermissions(signer, address, sent);
      void client.invalidateQueries({ queryKey: eventAdminKeys.access(signer.signer) });
      if (!mounted.current) return;
      const kept = saved.permissions.length !== sent.length || sent.some((permission) => !saved.permissions.includes(permission));
      setEditing(null);
      setToast({ message: kept ? adminCopy.permissions.serverKept : adminCopy.permissions.saveSuccess, tone: "success" });
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error && cause.message ? cause.message : adminCopy.permissions.saveError);
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  return (
    <>
      <h1 className={adminPageStyles.title}>{adminCopy.users.title}</h1>
      <UsersTable rows={rows} onAdd={() => open({ permissions: [] })} onEdit={(row) => open({ user: row, permissions: row.permissions })} />
      {editing ? (
        <PermissionsModal
          key={editing.user?.address ?? "add"}
          user={editing.user}
          permissions={editing.permissions}
          existing={(address) => listed(address)?.permissions}
          self={access.identity?.signer}
          busy={busy}
          feedback={error ? <Notice tone="error">{error}</Notice> : undefined}
          onClose={() => { setError(null); setEditing(null); }}
          onSubmit={(payload) => void save(payload)}
        />
      ) : null}
      <div role="status">
        {toast ? <p className={adminPageStyles.toast} data-tone={toast.tone}>{toast.message}</p> : null}
      </div>
    </>
  );
}
