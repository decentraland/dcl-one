import { redirect } from "react-router";

import { blogSignInReturnPath } from "@/features/auth/login/legacy-sign-in";
import { loginPath } from "@/features/auth/login/redirect";

import type { Route } from "./+types/blog.sign-in";

/** decentraland.org's `/blog/sign-in`: straight to the sign-in page, with the blog's return path rule. */
export function loader({ request }: Route.LoaderArgs) {
  throw redirect(loginPath(blogSignInReturnPath(new URL(request.url))));
}

/** Never rendered: the loader always redirects. */
export default function BlogSignIn() {
  return null;
}
