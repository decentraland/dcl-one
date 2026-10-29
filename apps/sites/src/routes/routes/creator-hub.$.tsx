import { redirect } from "react-router";

import { ensureSid, serializeSidCookie } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/creator-hub.$";

/** `/creator-hub/*` is the retired spelling of `/create/*`: one hub, one prefix. Pages that
    were renamed along the way map here; everything else keeps its path and query. */
const RENAMED: Record<string, string> = {
  manage: "worlds",
  "deploy-world": "publish",
  "deploy-alternative": "publish",
  "world-permissions": "admissions",
  gate: "admissions",
  "worlds-storage": "storage",
  "multiplayer-test": "preview",
};

export function createPathFor(url: URL, splat: string | undefined): string {
  const path = (splat ?? "").replace(/^\/+|\/+$/g, "");
  const [head, ...rest] = path ? path.split("/") : [];
  const mapped = head ? [RENAMED[head] ?? head, ...rest].join("/") : "";
  return `/create${mapped ? `/${mapped}` : ""}${url.search}`;
}

function moved({ request, params }: Route.LoaderArgs | Route.ActionArgs) {
  const { sid, created } = ensureSid(request);
  const url = new URL(request.url);
  const to = createPathFor(url, params["*"]);
  track("creator_builder_redirect", { from: url.pathname, to }, { sid });
  return redirect(to, {
    status: 308,
    headers: created ? { "Set-Cookie": serializeSidCookie(sid) } : undefined,
  });
}

export const loader = moved;
export const action = moved;

export default function CreatorHubMoved() {
  return null;
}
