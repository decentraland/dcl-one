import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";
import { afterEach, beforeEach, expect, vi } from "vitest";
import { primeRemoteFlags } from "../shop/discovery/lib/remoteFlags";
import { FIXTURE_NOW } from "../shop/market/fixtureNow";

configure({ asyncUtilTimeout: 10_000 });

// The shop fixtures carry absolute dates (bid and order expiries) relative to FIXTURE_NOW.
const atFixtureTime = () => expect.getState().testPath?.includes("/src/shop/") ?? false;
beforeEach(() => {
  if (atFixtureTime()) vi.useFakeTimers({ toFake: ["Date"], now: FIXTURE_NOW, shouldAdvanceTime: true });
});
afterEach(() => {
  if (atFixtureTime()) vi.useRealTimers();
});

// The shop reads rollout flags from Decentraland's live feature-flags service; every test starts with all
// of them off and primed, so none reaches the service and none changes when a flag is turned on there.
beforeEach(() => primeRemoteFlags({}));
