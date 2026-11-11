import { redirect } from "react-router";
import { legalPath, resolveLegalKey } from "@/data/legal";

import type { Route } from "./+types/legal.$doc";

export async function loader({ params }: Route.LoaderArgs) {
  const doc = params.doc ?? "";
  const key = doc === "credits" ? "credits" : resolveLegalKey(doc);
  throw redirect(key ? legalPath(key) : doc ? `/${doc}` : "/", 301);
}

export default function LegalDocRedirect() {
  return null;
}
