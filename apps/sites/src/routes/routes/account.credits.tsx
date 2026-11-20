import { useOutletContext } from "react-router";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CreditsSettings } from "@/features/account/CreditsSettings";

export function meta() {
  return [{ title: "Credits Settings | Decentraland" }, { name: "robots", content: "noindex" }];
}

export default function AccountCredits() {
  return <CreditsSettings identity={useOutletContext<AuthIdentity>()} />;
}
