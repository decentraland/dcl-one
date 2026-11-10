import { legacyAccountsRedirect } from "../lib/profile-loader.server";

import type { Route } from "./+types/profile.accounts.$address.$tab";

export const loader = ({ params }: Route.LoaderArgs) => legacyAccountsRedirect(params);
