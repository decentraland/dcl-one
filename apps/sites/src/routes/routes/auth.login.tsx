import { useLoaderData } from "react-router";

import { signInLayoutVariant } from "@/data/lib/auth/sign-in-flags.server";
import LoginPage from "@/features/auth/login/LoginPage";

import type { Route } from "./+types/auth.login";

export function meta(_args: Route.MetaArgs) {
  return [{ title: "Decentraland | Authorization" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  return {
    redirectTo: url.searchParams.get("redirectTo"),
    layoutVariant: await signInLayoutVariant(),
    // Magic's publishable key: public by design, but deployment configuration, so it comes from the environment.
    magicApiKey: process.env.MAGIC_API_KEY || null,
  };
}

export default function AuthLoginRoute() {
  const { redirectTo, layoutVariant, magicApiKey } = useLoaderData<typeof loader>();
  return <LoginPage redirectTo={redirectTo} layoutVariant={layoutVariant} magicApiKey={magicApiKey} />;
}
