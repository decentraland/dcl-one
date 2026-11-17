import { useLoaderData } from "react-router";

import { signInLayoutVariant } from "@/data/lib/auth/sign-in-flags.server";
import LoginPage from "@/features/auth/login/LoginPage";
import { requestedOption } from "@/features/auth/login/options";

import type { Route } from "./+types/auth.login";

export function meta(_args: Route.MetaArgs) {
  return [{ title: "Decentraland | Authorization" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  return {
    redirectTo: url.searchParams.get("redirectTo"),
    // the method the player already picked in the Explorer, started at once
    loginMethod: requestedOption(url.searchParams.get("loginMethod")),
    layoutVariant: await signInLayoutVariant(),
    // Magic's publishable key: public by design, but deployment configuration, so it comes from the environment.
    magicApiKey: process.env.MAGIC_API_KEY || null,
  };
}

export default function AuthLoginRoute() {
  const { redirectTo, layoutVariant, magicApiKey, loginMethod } = useLoaderData<typeof loader>();
  return <LoginPage redirectTo={redirectTo} layoutVariant={layoutVariant} magicApiKey={magicApiKey} loginMethod={loginMethod} />;
}
