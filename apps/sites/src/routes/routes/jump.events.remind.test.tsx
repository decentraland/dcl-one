// @vitest-environment node
// Node keeps the route module's server imports loadable; the DOM is installed per test.
import { builtinEnvironments } from "vitest/runtime";
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const auth = vi.hoisted(() => ({ identity: null as unknown }));
const remind = vi.hoisted(() => ({ toggle: vi.fn() }));

vi.mock("@/data/lib/catalyst/jump/jump.server", () => ({
  loadCreator: vi.fn(),
  loadJumpEvent: vi.fn(),
  loadJumpPlace: vi.fn(),
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));
vi.mock("@/features/jump/useRemindMe", () => ({
  useRemindMe: (identity: unknown, _eventId: string, onSignIn: () => void) => ({
    active: false,
    pending: false,
    failed: false,
    toggle: () => {
      remind.toggle();
      if (!identity) onSignIn();
    },
  }),
}));

import { toEventCard } from "@/data/lib/catalyst/jump/jump";
import { cardProps, eventActionState } from "@/features/jump/view";

import JumpEvents from "./jump.events._index";

const START = new Date(Date.now() + 86_400_000).toISOString();
const card = toEventCard({
  id: "d275788c-2e03-4163-b7aa-2d997f679936",
  name: "Watch Party",
  description: "Together.",
  image: null,
  user: null,
  user_name: "Host",
  coordinates: "0,0",
  next_start_at: START,
  next_finish_at: START,
  live: false,
  total_attendees: 0,
  deleted_by_user: false,
  deleted_by_admin: false,
} as never);
const loaderData = {
  eventId: "d275788c-2e03-4163-b7aa-2d997f679936",
  card: cardProps(card, null),
  state: eventActionState(card),
  shareUrl: "https://decentraland.org/jump/events?id=d275788c-2e03-4163-b7aa-2d997f679936",
  calendarHref: null,
  shareText: { title: "Watch Party", text: "Together." },
  launch: null,
};

function show() {
  const router = createMemoryRouter([{ id: "jump", path: "/jump/events", Component: JumpEvents as unknown as () => null, loader: () => loaderData }], {
    initialEntries: ["/jump/events"],
    hydrationData: { loaderData: { jump: loaderData } },
  });
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

let dom: Awaited<ReturnType<typeof builtinEnvironments.jsdom.setup>>;
beforeEach(async () => {
  dom = await builtinEnvironments.jsdom.setup(globalThis, { jsdom: { url: "https://example.org/jump/events" } });
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  auth.identity = null;
  remind.toggle.mockReset();
});
afterEach(async () => {
  cleanup();
  vi.unstubAllGlobals();
  await dom.teardown(globalThis);
});

describe("Remind me on an event's jump page", () => {
  it("sends a signed-out visitor to the sign-in page, to come back to this event", async () => {
    const view = show();
    const button = await view.findByRole("button", { name: /remind me/i });
    const assign = vi.fn();
    vi.stubGlobal("location", { pathname: "/jump/events", search: "?id=d275788c", hash: "", assign });
    await userEvent.click(button);
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fjump%2Fevents%3Fid%3Dd275788c");
  });

  it("does not ask a signed-in member to sign in", async () => {
    auth.identity = { signer: "0xab" };
    const view = show();
    const assign = vi.fn();
    vi.stubGlobal("location", { pathname: "/jump/events", search: "", hash: "", assign });
    await userEvent.click(await view.findByRole("button", { name: /remind me/i }));
    expect(remind.toggle).toHaveBeenCalledOnce();
    expect(assign).not.toHaveBeenCalled();
  });
});
