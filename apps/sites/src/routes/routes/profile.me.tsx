import { ownProfileRedirect } from "../lib/profile-loader.server";

import type { Route } from "./+types/profile.me";

export const loader = ({ request }: Route.LoaderArgs) => ownProfileRedirect(request);
