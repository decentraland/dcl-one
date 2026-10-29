import { fireEvent, render, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, expect, test, vi } from "vitest";
import { fetchProfile } from "../../data/catalyst/profile";
import { ChatBubble } from "./Chat";

vi.mock("../../data/catalyst/profile", async (original) => ({
  ...(await original<typeof import("../../data/catalyst/profile")>()),
  fetchProfile: vi.fn(),
}));

const alice = "0x1111111111111111111111111111111111111111";
afterEach(() => { vi.mocked(fetchProfile).mockReset(); });

test("a sender without a picture from the bridge shows their profile face", async () => {
  vi.mocked(fetchProfile).mockResolvedValue({ avatar: { snapshots: { face256: "bafyalicefacehash" } } } as never);
  const { container } = render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <ChatBubble line={{ id: "1", ts: Date.now(), sender: alice, message: "hi" }} name="Alice" />
    </QueryClientProvider>,
  );
  await waitFor(() => expect(container.querySelector("img")?.getAttribute("src")).toContain("bafyalicefacehash"));
  expect(fetchProfile).toHaveBeenCalledWith(alice, expect.anything());
});

test("a picture the bridge already has is used without asking for the profile", () => {
  const { container } = render(
    <QueryClientProvider client={new QueryClient()}>
      <ChatBubble line={{ id: "1", ts: Date.now(), sender: alice, message: "hi" }} name="Alice" members={[{ address: alice, name: "Alice", wearables: [], coords: "0,0", picture: "https://example.test/alice.png" }]} />
    </QueryClientProvider>,
  );
  expect(container.querySelector("img")).toHaveAttribute("src", "https://example.test/alice.png");
  expect(fetchProfile).not.toHaveBeenCalled();
});

test("when the same-origin face fails to load, the profile's own snapshot URL is tried", async () => {
  const cdn = "https://profile-images.decentraland.org/entities/bafyentity/face.png";
  vi.mocked(fetchProfile).mockResolvedValue({ avatar: { snapshots: { face256: cdn } } } as never);
  const { container } = render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <ChatBubble line={{ id: "1", ts: Date.now(), sender: alice, message: "hi" }} name="Alice" />
    </QueryClientProvider>,
  );
  await waitFor(() => expect(container.querySelector("img")?.getAttribute("src")).toMatch(/\/profile-images\/entities\/bafyentity\/face\.png$/));
  expect(container.querySelector("img")?.getAttribute("src")).not.toBe(cdn);
  fireEvent.error(container.querySelector("img")!);
  await waitFor(() => expect(container.querySelector("img")).toHaveAttribute("src", cdn));
  fireEvent.error(container.querySelector("img")!);
  await waitFor(() => expect(container.querySelector("img")).toBeNull());
  expect(container).toHaveTextContent("A");
});
