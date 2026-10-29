import { redirect } from "react-router";

import type { Route } from "./+types/create.worlds-storage";

/** `/create/worlds-storage` is the retired name of `/create/storage`. */
export async function loader({ request }: Route.LoaderArgs) {
  return redirect("/create/storage" + new URL(request.url).search, 308);
}

export default function CreateStorageMoved() {
  return null;
}
