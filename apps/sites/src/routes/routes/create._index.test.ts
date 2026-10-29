import { beforeEach, expect, it, vi } from "vitest";
import { loadCreateScreen } from "@/data/lib/screens/create.server";
import { loader, sdkWorkspaceHandoff } from "./create._index";

vi.mock("@/data/lib/screens/create.server", () => ({ loadCreateScreen: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({ sidLoader: vi.fn() }));

beforeEach(() => vi.clearAllMocks());

async function thrown(run: () => Promise<unknown>): Promise<unknown> {
  try {
    await run();
  } catch (error) {
    return error;
  }
  throw new Error("the loader returned instead of redirecting");
}

// The link `dcl-scene-cli start` prints is `/create?projectUrl=…#token=…`. The token lives in the
// fragment, which never reaches this server, and reaches the editor route only because a
// document navigation carries a fragment across a redirect whose Location has none (RFC 9110
// §10.2.2). Handling projectUrl in a clientLoader, or answering with a Location that has its own
// fragment, would drop the token with no other failing test: this one pins the shape.
it("hands /create?projectUrl= to the SDK workspace by a fragment-free document redirect", async () => {
  const request = new Request("https://sites.test/create?projectUrl=http%3A%2F%2F127.0.0.1%3A8000%2F&creator=0xabc");
  const response = await thrown(() => loader({ request } as never));
  expect(response).toBeInstanceOf(Response);
  const { status, headers } = response as Response;
  expect(status).toBe(302);
  const location = headers.get("location")!;
  expect(location).toBe("/create/scene-editor?source=sdk&projectUrl=http%3A%2F%2F127.0.0.1%3A8000%2F&from=home");
  expect(location).not.toContain("#");
  expect(new URL(location, "https://sites.test").searchParams.has("token")).toBe(false);
  expect(headers.get("cache-control")).toBe("private, no-store");
  expect(loadCreateScreen).not.toHaveBeenCalled();
});

it("only a projectUrl is a handoff, and a token in the query is not carried", () => {
  expect(sdkWorkspaceHandoff(new URL("https://sites.test/create"))).toBeNull();
  expect(sdkWorkspaceHandoff(new URL("https://sites.test/create?projectUrl=%20"))).toBeNull();
  const handoff = sdkWorkspaceHandoff(new URL("https://sites.test/create?projectUrl=http%3A%2F%2F127.0.0.1%3A8000&token=leaked"))!;
  expect(new URLSearchParams(handoff.split("?")[1]).get("token")).toBeNull();
  expect(new URLSearchParams(handoff.split("?")[1]).get("projectUrl")).toBe("http://127.0.0.1:8000");
});
