import { useOutletContext } from "react-router";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { NotificationsSettings } from "@/features/account/NotificationsSettings";

export function meta() {
  return [{ title: "Notifications | Decentraland" }, { name: "robots", content: "noindex" }];
}

export default function AccountNotifications() {
  return <NotificationsSettings identity={useOutletContext<AuthIdentity>()} />;
}
