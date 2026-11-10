import "@dcl/one-ui/foundations/foundations.css";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

import { profileLoader } from "../lib/profile-loader.server";
import { ProfileErrorBoundary, ProfileRoute, profileClientLoader } from "../lib/profile-route";

import type { Route } from "./+types/profile_.$address.$tab";

export const loader = ({ request, params }: Route.LoaderArgs) => profileLoader(request, params);
export const clientLoader = profileClientLoader;
export const ErrorBoundary = ProfileErrorBoundary;
export const handle = { agentMarkdown: "profileDetail" } satisfies AgentMarkdownHandle;

export default function ProfileAddressTab({ loaderData }: Route.ComponentProps) {
  return <ProfileRoute loaderData={loaderData} />;
}
