import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";
import { afterEach, beforeEach, expect, vi } from "vitest";
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
