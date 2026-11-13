import { ownProfileRedirect } from "../lib/profile-loader.server";

import type { Route } from "./+types/profile._index";

export const loader = ({ request }: Route.LoaderArgs) => ownProfileRedirect(request);
