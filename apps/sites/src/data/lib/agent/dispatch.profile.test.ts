import { dehydrate, QueryClient } from "@tanstack/react-query";
import type { EntryContext } from "react-router";
import { describe, expect, it } from "vitest";

import { mapProfile } from "../catalyst/social/profile";
import { profileKeys } from "../profile/queries";
import { renderAgentMarkdown } from "./dispatch.server";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

/** A profile document's router context, with the loader data the profile route returns. */
function context(profile: unknown, cached = true): EntryContext {
  const client = new QueryClient();
  if (cached) client.setQueryData(profileKeys.profile(A), profile);
  return {
    staticHandlerContext: {
      matches: [{ route: { id: "profile", handle: { agentMarkdown: "profileDetail" } } }],
      loaderData: { profile: { address: A, dehydratedState: dehydrate(client) } },
      statusCode: 200,
    },
  } as unknown as EntryContext;
}

describe("profile agent markdown", () => {
  it("describes the profile the loader dehydrated", () => {
    const profile = mapProfile(
      { name: "Alice", hasClaimedName: true, description: "", avatar: { snapshots: { face256: "" } } } as never,
      A,
    );

    const out = renderAgentMarkdown(context(profile));

    expect(out?.status).toBe(200);
    expect(out?.md).toContain("Alice");
  });

  it("describes a wallet with no profile yet as the empty profile the page shows", () => {
    const out = renderAgentMarkdown(context(null));

    expect(out?.md).toContain(A);
  });

  it("returns nothing when the profile was not dehydrated", () => {
    expect(renderAgentMarkdown(context(undefined, false))).toBeNull();
  });
});
