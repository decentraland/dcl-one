import "@dcl/one-ui/foundations/foundations.css";
import { HydrationBoundary } from "@tanstack/react-query";
import { useState } from "react";
import { isRouteErrorResponse, useParams } from "react-router";

import { CommunityErrorView } from "@/features/social/community/CommunityErrorView";
import { CommunityPage } from "@/features/social/community/CommunityPage";

import { communityLoader } from "../lib/community-loader.server";

import type { Route } from "./+types/social_.communities.$id";

export const loader = ({ request, context, params }: Route.LoaderArgs) => communityLoader(request, context, params);

/** Client navigations fetch through the browser client; only the first document carries server data. */
export async function clientLoader() {
  return null;
}

export function meta({ loaderData, error }: Route.MetaArgs) {
  if (!loaderData) {
    const notFound = isRouteErrorResponse(error) && error.status === 404;
    return [{ title: notFound ? "Community not found | Decentraland" : "Community | Decentraland" }];
  }
  return [
    { title: `${loaderData.title} | Decentraland` },
    ...(loaderData.description ? [{ name: "description", content: loaderData.description }] : []),
  ];
}

export function ErrorBoundary() {
  return <CommunityErrorView />;
}

export default function CommunityDetailRoute({ loaderData }: Route.ComponentProps) {
  const params = useParams();
  // After a client navigation there is no loader clock; the browser's stands in for it.
  const [now] = useState(() => loaderData?.now ?? Date.now());
  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      <CommunityPage key={params.id} id={params.id ?? ""} now={now} />
    </HydrationBoundary>
  );
}
