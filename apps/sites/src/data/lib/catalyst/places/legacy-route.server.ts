import { redirect } from "react-router";
import { CatalystError } from "../client";
import { resolveLegacyPlace } from "./destinations";

export async function legacyPlaceRedirect(request: Request, kind: "place" | "world", value: string) {
  try {
    const id = await resolveLegacyPlace(kind, value, { signal: request.signal });
    if (!id) throw new Response("Place not found", { status: 404 });
    return redirect(`/places/${encodeURIComponent(id)}${new URL(request.url).search}`);
  } catch (error) {
    if (error instanceof Response) throw error;
    if (error instanceof CatalystError && error.status === 404) throw new Response("Place not found", { status: 404 });
    throw new Response("Places are temporarily unavailable", { status: 503 });
  }
}
