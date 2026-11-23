import { dehydrate, QueryClient } from "@tanstack/react-query";
import type { EntryContext } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

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
  let out: ReturnType<typeof renderAgentMarkdown>;

  describe("when the loader dehydrated a profile", () => {
    beforeEach(() => {
      const profile = mapProfile(
        { name: "Alice", hasClaimedName: true, description: "", avatar: { snapshots: { face256: "" } } } as never,
        A,
      );
      out = renderAgentMarkdown(context(profile));
    });

    it("should answer with a 200 status", () => {
      expect(out?.status).toBe(200);
    });

    it("should describe the profile", () => {
      expect(out?.md).toContain("Alice");
    });
  });

  describe("when the wallet has no profile yet", () => {
    beforeEach(() => {
      out = renderAgentMarkdown(context(null));
    });

    it("should describe it as the empty profile the page shows", () => {
      expect(out?.md).toContain(A);
    });
  });

  describe("when the profile was not dehydrated", () => {
    beforeEach(() => {
      out = renderAgentMarkdown(context(undefined, false));
    });

    it("should return nothing", () => {
      expect(out).toBeNull();
    });
  });
});
