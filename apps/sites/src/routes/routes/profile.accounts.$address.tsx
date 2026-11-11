import { legacyAccountsRedirect } from "../lib/profile-loader.server";

import type { Route } from "./+types/profile.accounts.$address";

export const loader = ({ request, params }: Route.LoaderArgs) => legacyAccountsRedirect(request, params);
