import { redirect } from "react-router";

import type { Route } from "./+types/play_.app";

// The app document lives at /play/app.html, the path the engine's service worker isolates;
// earlier links to /play/app keep working.
export function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  return redirect(`/play/app.html${url.search}`, 301);
}
