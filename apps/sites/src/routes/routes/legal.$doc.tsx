import { redirect } from "react-router";
import { legalPath, resolveLegalKey } from "@/data/legal/nav";

import type { Route } from "./+types/legal.$doc";

export async function loader({ params }: Route.LoaderArgs) {
  const key = resolveLegalKey(params.doc ?? "");
  throw redirect(key ? legalPath(key) : "/", 301);
}

export default function LegalDocRedirect() {
  return null;
}
