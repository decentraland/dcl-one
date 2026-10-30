import { redirect } from "react-router";

import type { Route } from "./+types/create.deploy-world";

/** `/create/deploy-world` is the retired name of `/create/publish`. */
export async function loader({ request }: Route.LoaderArgs) {
  return redirect("/create/publish" + new URL(request.url).search, 308);
}

export default function CreatePublishMoved() {
  return null;
}
