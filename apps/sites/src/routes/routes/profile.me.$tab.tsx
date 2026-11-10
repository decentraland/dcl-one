import { ownProfileRedirect } from "../lib/profile-loader.server";

import type { Route } from "./+types/profile.me.$tab";

export const loader = ({ request, params }: Route.LoaderArgs) => ownProfileRedirect(request, params.tab);
