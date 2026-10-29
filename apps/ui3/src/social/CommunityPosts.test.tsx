import { expect, test, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import CommunityPosts from "./CommunityPosts";
import type { SocialDetail } from "../explorer/pages/Communities";

const viewer = "0x1111111111111111111111111111111111111111";
const detail: SocialDetail = {
  id: "club", community: { id: "club", name: "Post club", role: "moderator", membersCount: 3 }, members: [], places: [],
  posts: [
    { id: "mine", authorAddress: viewer, authorName: "Moderator", content: "My post", createdAt: null, likesCount: 0 },
    { id: "theirs", authorAddress: "0x2222222222222222222222222222222222222222", authorName: "Owner", content: "Their post", createdAt: null, likesCount: 0 },
  ],
};
const load = async () => ({ posts: detail.posts, total: detail.posts.length });
function setup(children: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return { client, ...render(children, { wrapper: ({ children }) => <QueryClientProvider client={client}>{children}</QueryClientProvider> }) };
}

test("a pending or failed publish keeps the draft and cannot submit twice", async () => {
  const user = userEvent.setup();
  let reject!: (error: Error) => void;
  const write = vi.fn(() => new Promise<void>((_, fail) => { reject = fail; }));
  setup(<CommunityPosts detail={detail} viewer={viewer} load={load} write={write} />);
  await user.click(screen.getByRole("button", { name: "New post" }));
  await user.type(screen.getByRole("textbox", { name: "Post text" }), "Keep this draft");
  await user.dblClick(screen.getByRole("button", { name: "Publish post" }));
  expect(write).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  await act(async () => reject(new Error("Storage unavailable")));
  expect(screen.getByRole("alert")).toHaveTextContent("Storage unavailable");
  expect(screen.getByRole("textbox", { name: "Post text" })).toHaveValue("Keep this draft");
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("textbox", { name: "Post text" })).toBeNull();
  expect(write).toHaveBeenCalledTimes(1);
});

test("a publish retried after a lost answer sends the same Idempotency-Key, and the failure refreshes the feed", async () => {
  const user = userEvent.setup();
  const feed = vi.fn(load);
  const write = vi.fn<(method: string, suffix: string, body?: object, headers?: Record<string, string>) => Promise<void>>()
    .mockRejectedValueOnce(new Error("Network connection lost"))
    .mockResolvedValue(undefined);
  setup(<CommunityPosts detail={detail} viewer={viewer} load={feed} write={write} />);
  await screen.findByText("My post");
  expect(feed).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole("button", { name: "New post" }));
  await user.type(screen.getByRole("textbox", { name: "Post text" }), "Once only");
  await user.click(screen.getByRole("button", { name: "Publish post" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Network connection lost");
  await waitFor(() => expect(feed).toHaveBeenCalledTimes(2));
  await user.click(screen.getByRole("button", { name: "Publish post" }));
  await waitFor(() => expect(screen.queryByRole("textbox", { name: "Post text" })).toBeNull());
  const keys = write.mock.calls.map(([method, suffix, body, headers]) => [method, suffix, body, headers?.["Idempotency-Key"]]);
  expect(keys).toEqual([["POST", "", { content: "Once only" }, expect.any(String)], ["POST", "", { content: "Once only" }, keys[0]![3]]]);

  await user.click(screen.getByRole("button", { name: "New post" }));
  await user.type(screen.getByRole("textbox", { name: "Post text" }), "Once only");
  await user.click(screen.getByRole("button", { name: "Publish post" }));
  await waitFor(() => expect(write).toHaveBeenCalledTimes(3));
  expect(write.mock.calls[2]![3]?.["Idempotency-Key"]).not.toBe(keys[0]![3]);
});

test("post controls follow owner, moderator, member and unknown roles", async () => {
  const write = vi.fn();
  const { rerender } = setup(<CommunityPosts detail={detail} viewer={viewer} load={load} write={write} />);
  await screen.findByText("My post");
  expect(within(screen.getByText("My post").closest("article")!).getByRole("button", { name: "Post actions" })).toBeInTheDocument();
  expect(within(screen.getByText("Their post").closest("article")!).queryByRole("button", { name: "Post actions" })).toBeNull();
  const as = (role: string) => rerender(<CommunityPosts detail={{ ...detail, community: { ...detail.community!, role } }} viewer={viewer} load={load} write={write} />);
  as("owner");
  expect(screen.getAllByRole("button", { name: "Post actions" })).toHaveLength(2);
  for (const role of ["member", "admin", "none", "banned"]) {
    as(role);
    expect(screen.queryByRole("button", { name: "Post actions" })).toBeNull();
    expect(screen.queryByRole("button", { name: "New post" })).toBeNull();
  }
  expect(screen.queryByRole("button", { name: "Like" })).toBeNull();
});

test("older pages load on request, keep offsets and deduplicate overlapping entries", async () => {
  const user = userEvent.setup();
  const page = vi.fn(async (offset: number) => ({
    posts: offset === 0 ? [detail.posts[0]!] : offset === 1 ? [detail.posts[0]!, detail.posts[1]!] : [{ ...detail.posts[0]!, id: "old", content: "Oldest post" }], total: 4,
  }));
  setup(<CommunityPosts detail={detail} viewer={viewer} load={page} write={vi.fn()} />);
  await screen.findByText("My post");
  expect(page.mock.calls.map(([offset]) => offset)).toEqual([0]);
  await user.click(screen.getByRole("button", { name: "Load older posts" }));
  await screen.findByText("Their post");
  expect(screen.getAllByText("My post")).toHaveLength(1);
  await user.click(screen.getByRole("button", { name: "Load older posts" }));
  await screen.findByText("Oldest post");
  expect(page.mock.calls.map(([offset]) => offset)).toEqual([0, 1, 3]);
  expect(screen.getAllByRole("article")).toHaveLength(3);
  expect(screen.queryByRole("button", { name: "Load older posts" })).toBeNull();
});

test("a refused refresh hides cached posts and retry requires a successful read", async () => {
  const user = userEvent.setup();
  const page = vi.fn(load);
  const { client } = setup(<CommunityPosts detail={detail} viewer={viewer} load={page} write={vi.fn()} />);
  await screen.findByText("My post");
  page.mockRejectedValueOnce(new Error("Membership required (401)"));
  await act(async () => { await client.invalidateQueries({ queryKey: ["web-social", viewer, detail.id, "posts"] }); });
  expect(await screen.findByRole("alert")).toHaveTextContent("Membership required");
  expect(screen.queryByRole("article")).toBeNull();
  await user.click(screen.getByRole("button", { name: "Retry posts" }));
  await screen.findByText("My post");
  expect(page).toHaveBeenCalledTimes(3);
});

test("editing preserves the draft after refusal and saves only the original post ID", async () => {
  const user = userEvent.setup();
  let reject!: (error: Error) => void;
  const write = vi.fn(() => new Promise<void>((_, fail) => { reject = fail; }));
  setup(<CommunityPosts detail={detail} viewer={viewer} load={load} write={write} />);
  const post = (await screen.findByText("My post")).closest("article")!;
  await user.click(within(post).getByRole("button", { name: "Post actions" }));
  await user.click(screen.getByRole("menuitem", { name: "Edit post" }));
  const form = screen.getByRole("form", { name: "Edit post" });
  const text = within(form).getByRole("textbox", { name: "Post text" });
  expect(text).toHaveValue("My post");
  await user.clear(text);
  expect(within(form).getByRole("button", { name: "Save changes" })).toBeDisabled();
  await user.type(text, "Revised words");
  await user.dblClick(within(form).getByRole("button", { name: "Save changes" }));
  expect(write).toHaveBeenCalledTimes(1);
  expect(write).toHaveBeenCalledWith("PATCH", "/mine", { content: "Revised words" });
  await act(async () => reject(Object.assign(new Error("Unauthorized"), { status: 401 })));
  expect(screen.getByRole("alert")).toHaveTextContent("no longer have permission");
  expect(text).toHaveValue("Revised words");
  write.mockResolvedValueOnce();
  await user.click(within(form).getByRole("button", { name: "Save changes" }));
  expect(write).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("form", { name: "Edit post" })).toBeNull();
});

test("cancelling an edit writes nothing and owners cannot edit somebody else's words", async () => {
  const user = userEvent.setup();
  const write = vi.fn();
  setup(<CommunityPosts detail={{ ...detail, community: { ...detail.community!, role: "owner" } }} viewer={viewer} load={load} write={write} />);
  const theirs = (await screen.findByText("Their post")).closest("article")!;
  await user.click(within(theirs).getByRole("button", { name: "Post actions" }));
  expect(screen.queryByRole("menuitem", { name: "Edit post" })).toBeNull();
  expect(screen.getByRole("menuitem", { name: "Delete post" })).toBeInTheDocument();
  await user.keyboard("{Escape}");
  await user.click(within(screen.getByText("My post").closest("article")!).getByRole("button", { name: "Post actions" }));
  await user.click(screen.getByRole("menuitem", { name: "Edit post" }));
  await user.type(screen.getByRole("textbox", { name: "Post text" }), " unsaved");
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(write).not.toHaveBeenCalled();
  expect(screen.getByText("My post")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "New post" }));
  expect(screen.getByRole("textbox", { name: "Post text" })).toHaveValue("");
});
