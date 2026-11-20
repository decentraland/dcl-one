// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

import CastConsole, { type LiveBindings } from "./CastConsole";
import type { GoLiveFn, RequestPermissionsFn, ResolveTokenFn } from "./machine";

const INFO = { placeName: "Test Plaza", placeId: "p1", location: "1,2", isWorld: false };
const CREDENTIALS = { url: "wss://livekit.test", token: "jwt", roomId: "room-1", identity: "Ada" };

function setup(opts: { requestPermissions?: RequestPermissionsFn } = {}) {
  const resolveToken = vi.fn<ResolveTokenFn>(async () => ({ info: INFO }));
  const requestPermissions = vi.fn<RequestPermissionsFn>(
    opts.requestPermissions ?? (async () => ({ granted: true })),
  );
  const goLive = vi.fn<GoLiveFn>(async () => CREDENTIALS);
  const shareScreen = vi.fn(async () => ({ published: true }));
  const endCast = vi.fn(async () => {});
  const live: LiveBindings = {
    goLive,
    stopShare: vi.fn(),
    bindShareEnded: () => () => {},
    bindConnectionLost: () => () => {},
    micOn: true,
    cameraOn: true,
    toggleMicrophone: vi.fn(),
    toggleCamera: vi.fn(),
    devicesBusy: false,
    audioBlocked: false,
    enableAudio: vi.fn(),
    participants: 1,
  };
  const Page = () => (
    <CastConsole
      trackCtx={{ sid: "sid", story: "cast/stream", variant: "live" }}
      token="stream-key"
      resolveToken={resolveToken}
      requestPermissions={requestPermissions}
      shareScreen={shareScreen}
      endCast={endCast}
      track={vi.fn()}
      live={live}
    />
  );
  const router = createMemoryRouter([{ path: "/cast/s/:token", Component: Page }], {
    initialEntries: ["/cast/s/stream-key"],
  });
  const { container } = render(<RouterProvider router={router} />);
  return { container, requestPermissions, goLive, shareScreen, endCast, live };
}

afterEach(cleanup);

describe("the Cast speaker page", () => {
  it("joins the room from one click on the visible JOIN NOW", async () => {
    const { container, requestPermissions, goLive } = setup();
    const join = await screen.findByRole("button", { name: "JOIN NOW" });
    fireEvent.change(screen.getByLabelText("Your name"), { target: { value: "Ada" } });
    fireEvent.click(join);

    await waitFor(() => expect(goLive).toHaveBeenCalledTimes(1));
    expect(requestPermissions).toHaveBeenCalledTimes(1);
    expect(goLive.mock.calls[0]![0]).toMatchObject({ token: "stream-key", identity: "Ada" });
    expect(await screen.findByRole("button", { name: "Leave stream" })).toBeTruthy();
    // The design is the whole UI: no second, unstyled set of controls.
    expect(container.querySelector(".cast-console__controls")).toBeNull();
  });

  it("retries blocked access from the toast and then joins without another click", async () => {
    let granted = false;
    const { goLive } = setup({ requestPermissions: async () => ({ granted }) });
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    const retry = await screen.findByRole("button", { name: "Retry" });
    expect(goLive).not.toHaveBeenCalled();

    granted = true;
    fireEvent.click(retry);
    await waitFor(() => expect(goLive).toHaveBeenCalledTimes(1));
  });

  it("drives the screen share, microphone, camera and leave from the live controls", async () => {
    const { shareScreen, endCast, live } = setup();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));

    fireEvent.click(await screen.findByRole("button", { name: "Share screen" }));
    await waitFor(() => expect(shareScreen).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole("button", { name: "Stop sharing" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Mute microphone" }));
    expect(live.toggleMicrophone).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Turn camera off" }));
    expect(live.toggleCamera).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Leave stream" }));
    await waitFor(() => expect(endCast).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Your cast has ended")).toBeTruthy();
  });
});
