import { redirect } from "react-router";

import { signInReturnPath } from "@/features/auth/login/legacy-sign-in";
import { loginPath } from "@/features/auth/login/redirect";

import type { Route } from "./+types/sign-in";

/** decentraland.org's `/sign-in`: straight to the sign-in page, coming back to `redirectTo` or this URL. */
export function loader({ request }: Route.LoaderArgs) {
  throw redirect(loginPath(signInReturnPath(new URL(request.url))));
}

/** Never rendered: the loader always redirects. */
export default function SignIn() {
  return null;
}
