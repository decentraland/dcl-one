import { afterEach, expect, it, vi } from "vitest";
import { loader } from "./internal.auth-config";

afterEach(() => vi.unstubAllEnvs());

it("exposes only the public project ID to the static explorer", async () => {
  vi.stubEnv("WALLETCONNECT_PROJECT_ID", "public-project");
  vi.stubEnv("THIRDWEB_SECRET_KEY", "server-only");
  const response = loader();
  expect(await response.json()).toEqual({ walletConnectProjectId: "public-project" });
  expect(response.headers.get("cache-control")).toBe("no-store");
});

it("reports an unconfigured deployment without inventing a project ID", async () => {
  vi.stubEnv("WALLETCONNECT_PROJECT_ID", "");
  expect(await loader().json()).toEqual({ walletConnectProjectId: "" });
});
