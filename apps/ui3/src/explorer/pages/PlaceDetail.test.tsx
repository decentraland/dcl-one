import { describe, expect, test, vi } from "vitest";
import { fireEvent, render as rtlRender, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement, ReactNode } from "react";

import PlaceDetail from "./PlaceDetail";
import { fetchCommunityProfiles } from "../../data/hooks/useCommunityProfiles";

// The node answers a profile lookup with the profiles it holds: none for these wallets.
vi.mock("../../data/hooks/useCommunityProfiles", () => ({ fetchCommunityProfiles: vi.fn(async () => ({})) }));

function render(ui: ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
  return rtlRender(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const place = {
  id: "p1",
  title: "Meta GamiMall",
  coords: "1,95",
  description: "Shops and games",
  image: "https://example.test/content/contents/bafkcover",
  contactName: "MetaDoge",
  owner: "0x797066a17F83425C1B4C7a8Cca52D19095520a52",
};
const JUMP = "/play/?position=1%2C95";
const mount = (over: Partial<Parameters<typeof PlaceDetail>[0]> = {}) => render(<PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} {...over} />);

describe("PlaceDetail", () => {
  test("with players in the scene, a desktop visitor gets the viewer, the chat panel and what to expect", () => {
    mount();
    const viewer = screen.getByRole("region", { name: "Meta GamiMall" });
    expect(within(viewer).getByRole("heading", { level: 1, name: "Meta GamiMall" })).toBeInTheDocument();
    expect(within(viewer).getByText("MetaDoge")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Jump into 1,95 (opens in a new tab)" })).toHaveAttribute("href", JUMP);
    expect(screen.getByRole("link", { name: "Jump in" })).toHaveAttribute("href", JUMP);
    expect(screen.getByRole("heading", { name: "In-World Chat" })).toBeInTheDocument();
    expect(screen.getByText("No messages yet.")).toBeInTheDocument();
    expect(screen.getByText("What to expect")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("the chat panel keeps one shell through its states: empty while the room is asked for or silent, and says when there is none", () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
    const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
    const { rerender } = rtlRender(<PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} chat={{ status: "loading", messages: [] }} />, { wrapper });
    const footer = () => screen.getByText(/in Decentraland to participate in the chat/).textContent;
    expect(screen.getByText("No messages yet.")).toBeInTheDocument();
    expect(footer()).toBe("Jump into Meta GamiMall in Decentraland to participate in the chat.");
    rerender(<PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} chat={{ status: "unavailable", messages: [] }} />);
    expect(screen.getByText("Chat is unavailable right now.")).toBeInTheDocument();
    rerender(<PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} chat={{ status: "ready", messages: [] }} />);
    expect(screen.getByText("No messages yet.")).toBeInTheDocument();
    const at = new Date(2026, 0, 1, 9, 5).getTime();
    const messages = [
      { sender: "brave-lynx", timestamp: at, message: "hello from the cast" },
      { sender: "0x797066a17F83425C1B4C7a8Cca52D19095520a52", timestamp: at, message: "gm" },
    ];
    rerender(<PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} chat={{ status: "ready", messages }} />);
    expect(screen.queryByText("No messages yet.")).toBeNull();
    expect(screen.getByText("brave-lynx")).toBeInTheDocument();
    expect(screen.getByText("hello from the cast")).toBeInTheDocument();
    expect(screen.getByText("gm")).toBeInTheDocument();
    // A wallet's name waits for its profile; the time is the viewer's clock.
    expect(screen.getByText("…")).toBeInTheDocument();
    expect(screen.getAllByText(new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }))).toHaveLength(2);
    expect(footer()).toBe("Jump into Meta GamiMall in Decentraland to participate in the chat.");
  });

  test("a wallet without a profile shows its short address once the lookup settles", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const messages = [{ sender: "0x797066a17F83425C1B4C7a8Cca52D19095520a52", timestamp: Date.now(), message: "gm" }];
    rtlRender(<QueryClientProvider client={client}><PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} chat={{ status: "ready", messages }} /></QueryClientProvider>);
    expect(await screen.findByText("0x7970...0a52")).toBeInTheDocument();
    expect(fetchCommunityProfiles).toHaveBeenCalledWith(["0x797066a17f83425c1b4c7a8cca52d19095520a52"], expect.anything());
  });

  test("a world with more than one scene picks the scene to jump into; with one there is no picker", () => {
    const onChange = vi.fn();
    const options = [{ value: "a", label: "Plaza (0,0)" }, { value: "b", label: "Arena (10,4)" }];
    const { unmount } = mount({ scenePicker: { options, value: "a", onChange } });
    fireEvent.change(screen.getByRole("combobox", { name: "Scene" }), { target: { value: "b" } });
    expect(onChange).toHaveBeenCalledWith("b");
    unmount();
    mount({ scenePicker: { options: options.slice(0, 1), value: "a", onChange } });
    expect(screen.queryByRole("combobox")).toBeNull();
  });

  test("explore the scene opens the explorer in the viewer, and stop closes it", () => {
    mount();
    fireEvent.click(screen.getByRole("button", { name: "Explore the scene" }));
    expect(screen.getByTitle("Meta GamiMall")).toHaveAttribute("src", JUMP);
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(screen.queryByTitle("Meta GamiMall")).toBeNull();
  });

  test("the creator's name opens the owner's profile", () => {
    const onCreator = vi.fn();
    mount({ onCreator });
    fireEvent.click(screen.getByRole("button", { name: "MetaDoge" }));
    expect(onCreator).toHaveBeenCalledWith("0x797066a17f83425c1b4c7a8cca52d19095520a52");
  });

  test("an empty scene shows the jump-in card without the live badges", () => {
    mount({ livePlayers: 0 });
    const dialog = screen.getByRole("dialog", { name: "Meta GamiMall" });
    expect(within(dialog).getByRole("button", { name: "Close" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Copy link" })).toBeInTheDocument();
    expect(within(dialog).getByRole("link", { name: "Jump in" })).toHaveAttribute("href", JUMP);
    expect(screen.queryByTestId("SensorsRoundedIcon")).toBeNull();
    expect(screen.queryByText("In-World Chat")).toBeNull();
  });

  test("a phone gets the jump-in card with the live player count", () => {
    mount({ mobile: true });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Live")).toBeInTheDocument();
    expect(screen.getByTestId("PermIdentityRoundedIcon").parentElement).toHaveTextContent("6");
  });

  test("the creator is the contact name, the foundation by its logo, and the title when the contact is junk", () => {
    const { unmount } = render(<PlaceDetail place={{ ...place, contactName: "decentraland foundation" }} livePlayers={0} />);
    expect(screen.getByText("Decentraland Foundation")).toBeInTheDocument();
    unmount();
    render(<PlaceDetail place={{ ...place, contactName: "sdk" }} livePlayers={0} />);
    expect(screen.getByRole("dialog")).toHaveTextContent("By Meta GamiMall");
  });

  test("a missing place says so", () => {
    render(<PlaceDetail notFound />);
    expect(screen.getByRole("heading", { name: "Scene not found" })).toBeInTheDocument();
  });
});
