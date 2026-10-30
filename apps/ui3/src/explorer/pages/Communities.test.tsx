import { expect, test, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Communities, { relativeDate, type SocialDetail } from "./Communities";

const NOW = Date.parse("2026-09-21T12:00:00Z");
const club = { id: "club", name: "Music Club", description: "Live sets every Friday.", membersCount: 4, privacy: "public", role: "member" };
const author = "0x1111111111111111111111111111111111111111";
const detail: SocialDetail = {
  id: "club",
  community: club,
  members: [{ memberAddress: author, name: "Nyx", role: "owner" }],
  places: [{ id: "plaza", title: "Club Plaza", addedBy: author, addedAt: "2026-09-02T12:00:00Z" }],
  posts: [
    { id: "new", authorAddress: author, authorName: "Nyx", content: "Newest word", createdAt: "2026-09-18T17:40:00Z", likesCount: 0 },
    { id: "old", authorAddress: author, authorName: "Nyx", content: "First word", createdAt: "2024-06-01T10:00:00Z", likesCount: 0 },
  ],
};

test("remote discovery keeps saved memberships and respects loading, search and pagination", async () => {
  const user = userEvent.setup(), onSearch = vi.fn(), onLoadMore = vi.fn();
  const match = { ...club, id: "remote", name: "River explorers", description: "A hiking club", role: "none" };
  const discovery = { communities: [match], filtered: true, isLoading: false, isError: false, onSearch, onLoadMore };
  const { rerender } = render(<Communities communities={[club]} initialSection="explore" discovery={discovery} />);
  expect(within(screen.getByRole("complementary", { name: "Communities" })).getByText("Music Club")).toBeInTheDocument();
  await user.type(screen.getByRole("searchbox", { name: "Find a community" }), "hiking");
  expect(onSearch).toHaveBeenLastCalledWith("hiking");
  expect(screen.getByRole("button", { name: /River explorers/ })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Load more communities" }));
  expect(onLoadMore).toHaveBeenCalledTimes(1);
  rerender(<Communities communities={[club]} initialSection="explore" discovery={{ ...discovery, loadingMore: true }} />);
  expect(screen.getByRole("button", { name: "Loading communities…" })).toBeDisabled();
  rerender(<Communities communities={[club]} initialSection="explore" discovery={{ ...discovery, isLoading: true }} />);
  expect(screen.queryByRole("button", { name: /River explorers/ })).toBeNull();
  expect(screen.queryByText("No communities match your search.")).toBeNull();
  rerender(<Communities communities={[club]} initialSection="explore" discovery={{ ...discovery, communities: [], onLoadMore: undefined }} />);
  expect(screen.getByText("No communities match your search.")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Load more communities" })).toBeNull();
});

test("membership review is reachable for moderators and disappears when their role is removed", async () => {
  const user = userEvent.setup();
  const managed = { ...detail, community: { ...club, role: "moderator" } };
  const { rerender } = render(<Communities detail={managed} initialCommunityId="club" communityRequestsContent={<p>Review applicants</p>} />);
  await user.click(within(screen.getByRole("complementary", { name: "Music Club" })).getByRole("button", { name: "Membership requests" }));
  expect(screen.getByText("Review applicants")).toBeInTheDocument();
  rerender(<Communities detail={detail} initialCommunityId="club" communityRequestsContent={<p>Review applicants</p>} />);
  expect(screen.queryByText("Review applicants")).toBeNull();
  expect(screen.queryByRole("button", { name: "Membership requests" })).toBeNull();
});

test("a failed detail refresh cannot display previously authorized private content", () => {
  const { rerender } = render(<Communities detail={detail} communities={[club]} initialCommunityId="club" />);
  expect(screen.getByText("Newest word")).toBeInTheDocument();
  rerender(<Communities detail={{ ...detail, isError: true }} communities={[club]} initialCommunityId="club" />);
  expect(screen.queryByText("Newest word")).toBeNull();
  expect(screen.getByText("Couldn't load this community.")).toBeInTheDocument();
});

test("private discovery metadata explains membership instead of claiming the feed is empty", async () => {
  const user = userEvent.setup();
  const privateClub = { ...club, privacy: "private", role: "none" };
  render(<Communities communities={[privateClub]} initialCommunityId="club" detail={{ ...detail, community: null, posts: [], members: [], places: [] }} membership={{ joined: false }} />);
  expect(screen.getByText("Join this community to read its announcements.")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /All announcements/ }));
  expect(screen.getByText("Join this community to view its announcements.")).toBeInTheDocument();
  expect(screen.queryByText("No announcements yet.")).toBeNull();
});

test("dates older than a month read as months or years, newer ones as the date", () => {
  expect(relativeDate("2026-08-21T12:00:00Z", NOW)).toBe("1 month ago");
  expect(relativeDate("2026-04-11T20:05:00Z", NOW)).toBe("5 months ago");
  expect(relativeDate("2025-09-01T00:00:00Z", NOW)).toBe("1 year ago");
  expect(relativeDate("2024-06-01T10:00:00Z", NOW)).toBe("2 years ago");
  expect(relativeDate("2026-09-18T17:40:00Z", NOW)).not.toMatch(/ago/);
});

test("compact social navigation reaches joined communities, channels, requests and creation", async () => {
  const user = userEvent.setup();
  const onSelect = vi.fn();
  const onCreate = vi.fn();
  render(<Communities communities={[club]} detail={detail} onSelect={onSelect} onCreate={onCreate}
    requestsCount={2} requestsContent={<p>Pending requests</p>} now={NOW} />);
  // jsdom does not apply viewport media queries; the published tour checks visibility/layout.
  const select = async (name: string) => {
    await user.click(screen.getByRole("button", { name: "Social navigation", hidden: true }));
    await user.click(screen.getByRole("menuitem", { name, hidden: true }));
  };
  await select("Music Club");
  expect(onSelect).toHaveBeenLastCalledWith("club");
  expect(screen.getByRole("heading", { name: "Music Club" })).toBeInTheDocument();
  await select("Members");
  expect(screen.getByText("Nyx")).toBeInTheDocument();
  await select("Overview");
  expect(screen.getByRole("heading", { name: "Music Club" })).toBeInTheDocument();
  await select("Invitations & requests (2)");
  expect(onSelect).toHaveBeenLastCalledWith(null);
  expect(screen.getByText("Pending requests")).toBeInTheDocument();
  await select("Explore communities");
  await user.type(screen.getByRole("searchbox", { name: "Find a community" }), "no matching club");
  expect(screen.queryByRole("button", { name: /Take a look/ })).toBeNull();
  await select("Create a community");
  expect(onCreate).toHaveBeenCalledOnce();
});

test("the rail offers Communities, Explore communities, Friends and Nearby, and a community opens on its summary", async () => {
  const user = userEvent.setup();
  render(<Communities communities={[club]} detail={detail} now={NOW} />);
  const rail = within(screen.getByRole("navigation", { name: "Sections and communities" }));
  expect(rail.getByRole("button", { name: "Communities" })).toHaveAttribute("aria-current", "page");
  for (const name of ["Explore communities", "Friends", "Nearby"]) expect(rail.getByRole("button", { name })).not.toHaveAttribute("aria-current");
  expect(rail.queryByRole("button", { name: /Events|Worlds/ })).toBeNull();

  await user.click(rail.getByRole("button", { name: "Explore communities" }));
  expect(screen.getByRole("heading", { name: "Your communities" })).toBeInTheDocument();

  await user.click(rail.getByRole("button", { name: "Music Club" }));
  expect(screen.getByRole("heading", { level: 1, name: "Music Club" })).toBeInTheDocument();
  expect(screen.getByText("Your role").nextElementSibling).toHaveTextContent("Member");
  const side = within(screen.getByRole("complementary", { name: "Music Club" }));
  expect(side.getAllByRole("button").map((button) => button.textContent?.replace(/\d+$/, ""))).toEqual(["Music Club", "General", "Announcements", "Hangouts", "Members"]);

  await user.click(side.getByRole("button", { name: "Hangouts" }));
  expect(screen.getByText("Club Plaza")).toBeInTheDocument();
  expect(screen.queryByRole("dialog", { name: /Hangouts/ })).toBeNull();
});

test("Hangouts only shows to members when places are shared, and always to moderators", () => {
  const empty = { ...detail, places: [] };
  const { rerender } = render(<Communities communities={[club]} detail={empty} initialCommunityId="club" now={NOW} />);
  const side = within(screen.getByRole("complementary", { name: "Music Club" }));
  expect(side.queryByRole("button", { name: "Hangouts" })).toBeNull();
  expect(side.getByRole("button", { name: "Announcements" })).toBeInTheDocument();

  const mod = { ...club, role: "moderator" };
  rerender(<Communities communities={[mod]} detail={{ ...empty, community: mod }} initialCommunityId="club" now={NOW} />);
  expect(side.getByRole("button", { name: "Hangouts" })).toBeInTheDocument();
});

test("a member sitting on Hangouts lands on General when the channel disappears", () => {
  const { rerender } = render(<Communities communities={[club]} detail={detail} initialCommunityId="club" initialChannel="hangouts" now={NOW} />);
  expect(screen.getByRole("heading", { level: 1, name: "Hangouts" })).toBeInTheDocument();
  expect(screen.getByText("Places shared by Music Club.")).toBeInTheDocument();

  rerender(<Communities communities={[club]} detail={{ ...detail, places: [] }} initialCommunityId="club" initialChannel="hangouts" now={NOW} />);
  const side = within(screen.getByRole("complementary", { name: "Music Club" }));
  expect(side.queryByRole("button", { name: "Hangouts" })).toBeNull();
  expect(side.getByRole("button", { name: "General" })).toHaveAttribute("aria-current", "page");
  expect(screen.queryByText("No places shared yet.")).toBeNull();
  expect(screen.getByRole("heading", { level: 1, name: "Welcome to General." })).toBeInTheDocument();
});

test("invitations and requests are a section of the panel, listed only while something is pending", async () => {
  const user = userEvent.setup();
  const content = <p>Pending requests</p>;
  const { rerender } = render(<Communities communities={[club]} requestsCount={0} requestsContent={content} now={NOW} />);
  expect(screen.queryByRole("button", { name: /Invitations & requests/ })).toBeNull();

  rerender(<Communities communities={[club]} requestsCount={2} requestsContent={content} now={NOW} />);
  await user.click(screen.getByRole("button", { name: /Invitations & requests/ }));
  const main = within(screen.getByRole("region", { name: "Invitations & requests" }));
  expect(main.getByText("Invitations & requests")).toBeInTheDocument();
  expect(main.getByText("Pending requests")).toBeInTheDocument();
  expect(screen.queryByRole("dialog", { name: /Notifications|requests/i })).toBeNull();
});

test("a member row's Add friend button reads Sending, then Requested, and a failure sits inline next to the button", async () => {
  const user = userEvent.setup();
  const stranger = { memberAddress: "0x2222222222222222222222222222222222222222", name: "Vex", role: "member" };
  let settle: { resolve: () => void; reject: (error: Error) => void } | null = null;
  const onAddFriend = vi.fn(() => new Promise<void>((resolve, reject) => { settle = { resolve, reject }; }));
  const { rerender } = render(<Communities communities={[club]} detail={{ ...detail, members: [...detail.members, stranger] }} initialCommunityId="club" initialChannel="members" onAddFriend={onAddFriend} now={NOW} />);
  const row = () => within(screen.getByText("Vex").closest("li")!);

  await user.click(row().getByRole("button", { name: "Add friend" }));
  expect(onAddFriend).toHaveBeenCalledWith(stranger.memberAddress);
  expect(row().getByText("Sending…")).toBeInTheDocument();
  expect(row().queryByRole("button")).toBeNull();
  await act(async () => settle!.reject(new Error("No reply from the server. Try again.")));
  expect(row().getByRole("alert")).toHaveTextContent("No reply from the server. Try again.");
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  expect(row().getByRole("button", { name: "Add friend" })).toBeInTheDocument();

  await user.click(row().getByRole("button", { name: "Add friend" }));
  expect(row().queryByRole("alert")).toBeNull();
  await act(async () => settle!.resolve());
  expect(row().getByText("Requested")).toBeInTheDocument();

  rerender(<Communities communities={[club]} detail={{ ...detail, members: [...detail.members, stranger] }} friends={[{ address: stranger.memberAddress, name: "Vex", online: true, where: "" }]} initialCommunityId="club" initialChannel="members" onAddFriend={onAddFriend} now={NOW} />);
  expect(row().getByText("Friend")).toBeInTheDocument();
});

test("a member already requested per the friends push reads Requested without a click, and does not re-send", async () => {
  const user = userEvent.setup();
  const stranger = { memberAddress: "0x2222222222222222222222222222222222222222", name: "Vex", role: "member" };
  const onAddFriend = vi.fn(() => Promise.resolve());
  render(<Communities communities={[club]} detail={{ ...detail, members: [...detail.members, stranger] }} sentRequests={[stranger.memberAddress.toUpperCase()]} initialCommunityId="club" initialChannel="members" onAddFriend={onAddFriend} now={NOW} />);
  const row = within(screen.getByText("Vex").closest("li")!);
  expect(row.getByText("Requested")).toBeInTheDocument();
  expect(row.queryByRole("button", { name: "Add friend" })).toBeNull();
  await user.click(row.getByText("Requested"));
  expect(onAddFriend).not.toHaveBeenCalled();
});

test("announcements read oldest first and carry the full date on hover", async () => {
  const user = userEvent.setup();
  render(<Communities communities={[club]} detail={detail} initialCommunityId="club" now={NOW} />);
  await user.click(screen.getByRole("button", { name: "Announcements" }));
  const log = within(screen.getByRole("log", { name: "Announcements" }));
  const posts = log.getAllByRole("article");
  expect(posts.map((post) => within(post).getByRole("paragraph").textContent)).toEqual(["First word", "Newest word"]);
  expect(within(posts[0]!).getByText("2 years ago")).toBeInTheDocument();
  expect(posts[0]).toHaveAttribute("title", expect.stringContaining("2024"));
  expect(screen.getByText("Only moderators can post here.")).toBeInTheDocument();
});

test("nearby shows the map and, with nobody around, points at live events by attendance with Jump in focused", async () => {
  const user = userEvent.setup();
  const onJumpEvent = vi.fn();
  const events = [
    { id: "quiet", name: "Quiet Meetup", attendees: 3, live: true },
    { id: "later", name: "Tomorrow Show", attendees: 90, live: false },
    { id: "party", name: "Plaza Party", attendees: 42, live: true },
  ];
  const { rerender } = render(<Communities communities={[club]} initialSection="nearby" nearbyMap={<p>Map here</p>} liveEventsLoading onJumpEvent={onJumpEvent} now={NOW} />);
  expect(screen.getByText("Map here")).toBeInTheDocument();
  expect(within(screen.getByRole("region", { name: "Find people" })).getByRole("status")).toHaveTextContent("Looking for live events");
  expect(screen.queryByText("No live events right now.")).toBeNull();
  rerender(<Communities communities={[club]} initialSection="nearby" nearbyMap={<p>Map here</p>} liveEvents={events} onJumpEvent={onJumpEvent} now={NOW} />);
  const find = within(screen.getByRole("region", { name: "Find people" }));
  expect(find.getAllByRole("listitem").map((li) => within(li).getByRole("button", { name: /attending/ }).textContent)).toEqual(["Plaza Party42 attending", "Quiet Meetup3 attending"]);
  const jumps = find.getAllByRole("button", { name: "Jump in" });
  expect(jumps[0]).toHaveFocus();
  expect(onJumpEvent).not.toHaveBeenCalled();
  await user.click(jumps[0]!);
  expect(onJumpEvent).toHaveBeenCalledWith("party");
  expect(screen.queryByRole("region", { name: "Around you" })).toBeNull();

  const stranger = { address: "0x2222222222222222222222222222222222222222", name: "Vex", coords: "12,-4" };
  rerender(<Communities communities={[club]} initialSection="nearby" nearby={[stranger]} liveEvents={events} onJumpEvent={onJumpEvent} now={NOW} />);
  expect(screen.queryByRole("region", { name: "Find people" })).toBeNull();
  expect(within(screen.getByRole("region", { name: "Around you" })).getByText("Vex")).toBeInTheDocument();
});
