import { RouterContextProvider } from "react-router";
import { afterEach, expect, it, vi } from "vitest";

import { markPublicResponse } from "@/core/lib/telemetry/session";

import { loader } from "./root";

afterEach(() => vi.unstubAllEnvs());

const load = async () => {
  const context = new RouterContextProvider();
  markPublicResponse(context);
  return loader({ request: new Request("https://example.test/"), params: {}, context } as never);
};

it("hands the page the build revision the image sets", async () => {
  vi.stubEnv("COMMIT_HASH", "abc123");
  expect((await load()).commitHash).toBe("abc123");
});

it("hands the page no revision when the image sets none", async () => {
  vi.stubEnv("COMMIT_HASH", "");
  expect((await load()).commitHash).toBe("");
});

it("hands the page the Pulse host the deployment sets", async () => {
  vi.stubEnv("PUBLIC_PULSE_SERVER", "pulse.example.test:7743");
  expect((await load()).pulseServer).toBe("pulse.example.test:7743");
});

it("hands the page no Pulse host when the deployment sets none", async () => {
  vi.stubEnv("PUBLIC_PULSE_SERVER", "");
  expect((await load()).pulseServer).toBe("");
});
