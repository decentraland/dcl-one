import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

import { profileLoader } from "../lib/profile-loader.server";
import { ProfileErrorBoundary, ProfileRoute, profileClientLoader } from "../lib/profile-route";

import type { Route } from "./+types/profile_.$address_.$tab";

export const loader = ({ request, context, params }: Route.LoaderArgs) => profileLoader(request, context, params);
export const clientLoader = profileClientLoader;
export const ErrorBoundary = ProfileErrorBoundary;
export const handle = { agentMarkdown: "profileDetail" } satisfies AgentMarkdownHandle;

export default function ProfileAddressTab({ loaderData }: Route.ComponentProps) {
  return <ProfileRoute loaderData={loaderData} />;
}
