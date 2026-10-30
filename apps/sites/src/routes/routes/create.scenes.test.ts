import { beforeEach, expect, it, vi } from "vitest";

import { handleStore, type ProjectMeta } from "@/data/lib/fs/handle-store";
import { listServerDrafts } from "@/data/lib/catalyst/creator-hub/scene-drafts-client";
import { listDraftCards } from "./create.scenes";

vi.mock("@/data/lib/fs/handle-store", async (original) => ({
  ...await original<object>(),
  handleStore: { list: vi.fn() },
}));
vi.mock("@/data/lib/catalyst/creator-hub/scene-drafts-client", () => ({ listServerDrafts: vi.fn() }));

const LOCAL: ProjectMeta = { slug: "plaza", title: "Plaza", updatedAt: 1 } as ProjectMeta;

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(handleStore.list).mockResolvedValue([LOCAL]);
});

it("a draft listing the node could not answer is a failure next to the browser's scenes, not an empty list", async () => {
  vi.mocked(listServerDrafts).mockRejectedValue(new Error("The node could not list your saved scenes (503)."));
  const listed = await listDraftCards();
  expect(listed.failed).toBe(true);
  expect(listed.cards.map((c) => c.slug)).toEqual(["plaza"]);

  vi.mocked(listServerDrafts).mockResolvedValue([]);
  expect(await listDraftCards()).toMatchObject({ failed: false, cards: [{ slug: "plaza" }] });

  vi.mocked(listServerDrafts).mockResolvedValue([{ id: "plaza", version: 1, updatedAt: 1, title: "Plaza" }, { id: "market", version: 1, updatedAt: 1, title: "Market" }]);
  expect(await listDraftCards()).toMatchObject({ failed: false, cards: [{ slug: "plaza", serverDraft: true }, { slug: "market", serverDraft: true }] });
});
