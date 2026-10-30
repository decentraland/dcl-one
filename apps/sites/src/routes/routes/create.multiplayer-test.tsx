import { redirect } from "react-router";

import type { Route } from "./+types/create.multiplayer-test";

/** `/create/multiplayer-test` is the retired name of `/create/preview`. */
export async function loader({ request }: Route.LoaderArgs) {
  return redirect("/create/preview" + new URL(request.url).search, 308);
}

export default function CreatePreviewMoved() {
  return null;
}
