import { AdminTabsBar, type AdminTab } from "@dcl/one-ui/apps/sites";

import { useEventAdmin } from "./useEventAdmin";

/** The admin bar for an account with any event permission; nothing for anyone else. */
export function EventAdminTabs({ active }: { active: AdminTab }) {
  const access = useEventAdmin();
  if (!access.isAdmin) return null;
  return <AdminTabsBar active={active} showPending={access.canSeePending} showUsers={access.canManageUsers} />;
}
