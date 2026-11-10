import { legacyAccountsRedirect } from "../lib/profile-loader.server";

import type { Route } from "./+types/profile.accounts.$address";

export const loader = ({ params }: Route.LoaderArgs) => legacyAccountsRedirect(params);
