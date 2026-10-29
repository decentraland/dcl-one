import { ensureSid } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/internal.page-not-found";

const MAX_LINE = 2000;
const STORY = "sites-error";

function clip(v: unknown): string {
  if (typeof v !== "string") return "";
  return v.length > MAX_LINE ? v.slice(0, MAX_LINE) : v;
}

// POST only: an explicit GET answer keeps React Router's "no loader" error, and the
// stack it adds in development, out of the response, so dev and prod answer alike.
export function loader() {
  return Response.json({ ok: false, error: "method not allowed" }, { status: 405, headers: { allow: "POST" } });
}

export async function action({ request }: Route.ActionArgs) {
  if (request.method !== "POST") {
    return Response.json({ ok: false, error: "method not allowed" }, { status: 405 });
  }

  let parsed: unknown;
  try {
    parsed = await request.json();
  } catch {
    return new Response(null, { status: 204 });
  }

  const r = (parsed ?? {}) as Record<string, unknown>;
  const path = clip(r.path).trim();
  if (!path.startsWith("/")) {
    return new Response(null, { status: 204 });
  }

  try {
    const { sid } = ensureSid(request);
    track(
      "page_not_found",
      {
        path,
        referrer: clip(r.referrer),
        spa: true,
        ua: request.headers.get("user-agent") ?? "",
      },
      { sid, story: STORY },
    );
  } catch {
  }
  return Response.json({ ok: true }, { status: 202 });
}
