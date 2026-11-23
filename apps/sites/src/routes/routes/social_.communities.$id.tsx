import "@dcl/one-ui/foundations/foundations.css";
import { HydrationBoundary } from "@tanstack/react-query";
import { useState } from "react";
import { isRouteErrorResponse, useParams, type ShouldRevalidateFunctionArgs } from "react-router";

import { EventsLocaleProvider } from "@/features/events/locale";
import { communityHref } from "@/features/social/community/view";
import { CommunityErrorPage } from "@/pages/social/CommunityErrorPage";
import { CommunityPage } from "@/pages/social/CommunityPage";

import { communityLoader } from "../lib/community-loader.server";
import { absoluteImage, absoluteUrl, notFoundMeta, oneLine, publicBase, socialMeta } from "../lib/social-meta";

import type { Route } from "./+types/social_.communities.$id";

export const loader = ({ request, context, params }: Route.LoaderArgs) => communityLoader(request, context, params);

/** Client navigations fetch through the browser client; only the first document carries server data. */
export async function clientLoader() {
  return null;
}

/**
 * Opening or closing an event's detail (`?event=`) changes only the query. Revalidating would swap the
 * first document's data for the client loader's nothing, and with it the page's title and card.
 */
export function shouldRevalidate({
  currentUrl,
  nextUrl,
  formMethod,
  defaultShouldRevalidate,
}: ShouldRevalidateFunctionArgs) {
  // A submission still revalidates, should the route ever take one.
  if (formMethod) return defaultShouldRevalidate;
  return currentUrl.pathname === nextUrl.pathname ? false : defaultShouldRevalidate;
}

/**
 * The community's link-preview card, as decentraland.org's: its description, else who owns it, else
 * an invitation; the generic communities card when the community could not be read here.
 */
export function meta({ loaderData, error, params, matches }: Route.MetaArgs) {
  if (isRouteErrorResponse(error) && error.status === 404) return notFoundMeta();
  const base = publicBase(matches);
  const url = params.id ? absoluteUrl(base, communityHref(params.id)) : undefined;
  if (!loaderData) {
    return socialMeta({
      title: "Communities in Decentraland | Decentraland",
      description: "Connect with players, join voice chats, and discover what the Decentraland community is up to.",
      url,
    });
  }
  return socialMeta({
    title: `${loaderData.title} | Decentraland`,
    description:
      oneLine(loaderData.description) ??
      (loaderData.ownerName ? `A Decentraland community by ${loaderData.ownerName}.` : "Join this Decentraland community."),
    image: absoluteImage(base, loaderData.image),
    url,
  });
}

export function ErrorBoundary() {
  return <CommunityErrorPage />;
}

export default function CommunityDetailRoute({ loaderData }: Route.ComponentProps) {
  const params = useParams();
  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      {/* An event's detail follows the visitor's language, as on the events page. */}
      <EventsLocaleProvider>
        {/* Keyed by community, so moving to another one starts from that navigation's own data. */}
        <CommunityRoutePage key={params.id} id={params.id ?? ""} loaderData={loaderData} />
      </EventsLocaleProvider>
    </HydrationBoundary>
  );
}

function CommunityRoutePage({ id, loaderData }: { id: string; loaderData: Route.ComponentProps["loaderData"] }) {
  // After a client navigation there is no loader clock, nor session; the browser's clock stands in.
  const [first] = useState(() => ({ now: loaderData?.now ?? Date.now(), sid: loaderData?.sid ?? "" }));
  return <CommunityPage id={id} now={first.now} sid={first.sid} />;
}
