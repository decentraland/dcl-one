import { data, redirect } from "react-router";

import { accountHostRedirect } from "@/data/lib/account/confirmation";

import type { Route } from "./+types/account_.$";

/** A path under `/account` that names no section; the root boundary renders the not-found page. */
export function loader({ request }: Route.LoaderArgs) {
  const moved = accountHostRedirect(request);
  if (moved) throw redirect(moved);
  throw data(null, { status: 404, statusText: "Not Found" });
}

/** Never rendered: the loader always throws. Exporting a page keeps this a document route, not a resource. */
export default function AccountNotFound() {
  return null;
}
