import { redirect } from "react-router";
import type { Route } from "./+types/create.activity";

// Activity has been retired. Keep bookmarked links useful without loading its upstream data.
export function loader({ request }: Route.LoaderArgs) {
  const address = new URL(request.url).searchParams.get("address")?.trim();
  const search = address ? "?" + new URLSearchParams({ address }).toString() : "";
  return redirect("/create/metrics" + search, 308);
}
