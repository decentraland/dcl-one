import { useOutletContext } from "react-router";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { WalletsSettings } from "@/features/account/WalletsSettings";

export function meta() {
  return [{ title: "Wallets | Decentraland" }, { name: "robots", content: "noindex" }];
}

export default function AccountWallets() {
  return <WalletsSettings identity={useOutletContext<AuthIdentity>()} />;
}
