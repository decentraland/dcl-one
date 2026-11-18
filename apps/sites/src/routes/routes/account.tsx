import "@dcl/one-ui/foundations/foundations.css";
import { Outlet, useLocation } from "react-router";

import { AccountLayout, type AccountSection } from "@/features/account/AccountLayout";

import { ACCOUNT_HEADERS, accountLoader } from "../lib/account-loader.server";

import type { Route } from "./+types/account";

export const loader = ({ request, context }: Route.LoaderArgs) => accountLoader(request, context);
export const headers = () => ACCOUNT_HEADERS;

export function meta() {
  return [{ title: "Account Settings | Decentraland" }, { name: "robots", content: "noindex" }];
}

function sectionOf(pathname: string): AccountSection | null {
  const section = pathname.replace(/\/+$/, "").split("/")[2];
  return section === "notifications" || section === "credits" ? section : null;
}

/** `/account`: the signed-in member's settings; each section reads its data with the browser's session. */
export default function AccountRoute() {
  const { pathname } = useLocation();
  return (
    <AccountLayout active={sectionOf(pathname)}>{(identity) => <Outlet context={identity} />}</AccountLayout>
  );
}
