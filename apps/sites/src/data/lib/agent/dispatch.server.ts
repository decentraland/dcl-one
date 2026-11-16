import type { EntryContext } from "react-router";

import type { DehydratedState } from "@tanstack/react-query";

import { LEGAL_DOCS, isLegalKey } from "../../legal";
import { toAgentLegalDoc } from "../../legal/agent";

import { placesFromDehydrated } from "../catalyst/places/discover";
import { emptyProfile, type ProfileVM } from "../catalyst/social/profile";
import { profileKeys } from "../profile/queries";

import {
  blogIndexToMarkdown,
  blogPostToMarkdown,
  communityToMarkdown,
  discoverToMarkdown,
  eventDetailToMarkdown,
  legalDocToMarkdown,
  placeToMarkdown,
  placesIndexToMarkdown,
  profileToMarkdown,
  shopPageToMarkdown,
  whatsOnToMarkdown,
  type AgentMarkdownKey,
} from "./markdown";

const SERIALIZERS: Record<AgentMarkdownKey, (data: unknown) => string | null> = {
  // A 404 or 503 blog page is not a list of 0 posts: null keeps the HTML page with its status and headers.
  blogIndex: (data) => {
    const d = data as { state?: string; posts?: Parameters<typeof blogIndexToMarkdown>[0] } | undefined;
    return d?.state === "ok" && d.posts ? blogIndexToMarkdown(d.posts) : null;
  },
  blogPost: (data) => {
    const d = data as { state?: string; post?: Parameters<typeof blogPostToMarkdown>[0] | null } | undefined;
    return d?.state === "ok" && d.post ? blogPostToMarkdown(d.post) : null;
  },
  legalDoc: (data) => {
    const d = data as { doc?: string } | undefined;
    return d?.doc && isLegalKey(d.doc)
      ? legalDocToMarkdown(toAgentLegalDoc(LEGAL_DOCS[d.doc]))
      : null;
  },
  placesIndex: (data) => {
    const d = data as { dehydratedState?: DehydratedState } | undefined;
    return placesIndexToMarkdown(placesFromDehydrated(d?.dehydratedState));
  },
  placeDetail: (data) => {
    const d = data as { place?: Parameters<typeof placeToMarkdown>[0] | null } | undefined;
    return d?.place ? placeToMarkdown(d.place) : null;
  },
  whatsOn: (data) => {
    const d = data as Parameters<typeof whatsOnToMarkdown>[0] | undefined;
    return d ? whatsOnToMarkdown(d) : null;
  },
  eventDetail: (data) => {
    const d = data as { event?: Parameters<typeof eventDetailToMarkdown>[0] | null } | undefined;
    return d?.event ? eventDetailToMarkdown(d.event) : null;
  },
  shopPage: (data) => {
    const d = data as Parameters<typeof shopPageToMarkdown>[0] | null | undefined;
    return d?.seo ? shopPageToMarkdown(d) : null;
  },
  profileDetail: (data) => {
    // The profile reaches the page as the loader's dehydrated profile query; a wallet with no
    // profile yet is described as the empty profile the page shows.
    const d = data as
      | { address?: string; dehydratedState?: { queries?: { queryKey: unknown; state: { data?: unknown } }[] } }
      | undefined;
    if (!d?.address) return null;
    const key = JSON.stringify(profileKeys.profile(d.address));
    const entry = d.dehydratedState?.queries?.find((q) => JSON.stringify(q.queryKey) === key);
    if (!entry) return null;
    const profile = (entry.state.data as ProfileVM | null | undefined) ?? emptyProfile(d.address);
    return profileToMarkdown(profile);
  },
  discover: (data) => {
    const d = data as { content?: Parameters<typeof discoverToMarkdown>[0] | null } | undefined;
    return d?.content ? discoverToMarkdown(d.content) : null;
  },
  communityDetail: (data) => {
    const d = data as { detail?: Parameters<typeof communityToMarkdown>[0] | null } | undefined;
    return d?.detail && d.detail.source === "live" ? communityToMarkdown(d.detail) : null;
  },
};

type MatchLike = { route: { id: string; handle?: unknown } };

export function renderAgentMarkdown(
  routerContext: EntryContext,
): { md: string; status: number } | null {
  const ctx = routerContext.staticHandlerContext;
  if (!ctx) return null;

  const matches = (ctx.matches ?? []) as unknown as MatchLike[];
  const loaderData = ctx.loaderData as Record<string, unknown>;

  for (let i = matches.length - 1; i >= 0; i--) {
    const route = matches[i].route;
    const handle = route.handle as { agentMarkdown?: AgentMarkdownKey } | undefined;
    const key = handle?.agentMarkdown;
    if (!key) continue;

    const serialize = SERIALIZERS[key];
    if (!serialize) return null;

    const md = serialize(loaderData[route.id]);
    if (md == null) return null;
    return { md, status: ctx.statusCode ?? 200 };
  }

  return null;
}
