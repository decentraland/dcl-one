import { dehydrate, QueryClient, type DehydratedState } from "@tanstack/react-query";
import { data, type RouterContextProvider } from "react-router";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { sidLoader } from "@/core/lib/telemetry/session";
import { CommunityNotFoundError } from "@/data/lib/catalyst/social/community-detail";
import { communityQuery, eventsQuery, membersQuery } from "@/data/lib/community/queries";
import { isRouteFatal, type CommunityErrorReason } from "@/features/social/community/errors";

import { OPTIONAL_READS_BUDGET_MS, withinBudget } from "./profile-loader.server";

export type CommunityLoaderData = {
  sid: string;
  id: string;
  /** The loader's clock, so event times read the same on the server and at hydration. */
  now: number;
  title: string;
  description: string;
  dehydratedState: DehydratedState;
};

/** The body of the status-bearing response the route's boundary renders on a document request. */
export type CommunityErrorData = { reason: CommunityErrorReason };

/**
 * `/social/communities/:id`. The community is required: an unknown or malformed id is 404 and a
 * social API that fails is 503. A public community's first page of members and of events is
 * optional: past the budget, or on a failure, the browser reads them. A private community's are
 * the member's own, read signed in the browser.
 */
export async function communityLoader(
  request: Request,
  context: Readonly<RouterContextProvider>,
  params: { id?: string },
) {
  const id = params.id ?? "";
  const session = sidLoader(request, context);
  // A visitor already gone is not read for; one who leaves mid-load stops the reads.
  request.signal.throwIfAborted();
  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  const cancel = () => void client.cancelQueries();
  request.signal.addEventListener("abort", cancel, { once: true });
  let title: string;
  let description: string;
  try {
    const community = await client.fetchQuery(communityQuery(id));
    title = community.name;
    // The page shows a private community's description to members only, so its meta leaves it out.
    description = community.privacy === "private" ? "" : (community.description ?? "");
    if (community.privacy === "public") {
      await withinBudget(
        Promise.all([client.prefetchInfiniteQuery(membersQuery(id)), client.prefetchInfiniteQuery(eventsQuery(id))]),
        OPTIONAL_READS_BUDGET_MS,
      );
    }
  } catch (error) {
    // Nothing is dehydrated on a failure, so any read still running is cancelled here.
    client.clear();
    if (error instanceof CommunityNotFoundError) {
      throw data({ reason: "not-found" } satisfies CommunityErrorData, { status: 404 });
    }
    // A social API failure is 503; anything else is a bug and stays a 500.
    if (!isRouteFatal(error)) throw error;
    throw data({ reason: "unavailable" } satisfies CommunityErrorData, { status: 503 });
  } finally {
    request.signal.removeEventListener("abort", cancel);
  }
  // Only settled successful queries are dehydrated, so a read still running past the budget is not.
  const dehydratedState = dehydrate(client);
  client.clear();
  return session.wrap({
    sid: session.sid,
    id,
    now: Date.now(),
    title,
    description,
    dehydratedState,
  } satisfies CommunityLoaderData);
}
