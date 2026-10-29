import { afterEach, describe, expect, it, vi } from "vitest";

import { canReadSceneErrors } from "./scene-errors.server";

const WALLET = "0x1234567890123456789012345678901234567890";
const OTHER = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd";

afterEach(() => vi.unstubAllGlobals());

describe("scene diagnostics authorization", () => {
  it("requires a current Genesis deploy permission", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({
      owner: false,
      operator: false,
      updateOperator: false,
      updateManager: false,
      approvedForAll: false,
    })));
    vi.stubGlobal("fetch", fetchMock);
    expect(await canReadSceneErrors(WALLET, "genesis", "1,2")).toBe(false);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(await canReadSceneErrors(WALLET, "genesis", "not-a-parcel")).toBe(false);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("lets the World owner read and refuses another wallet", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ owner: WALLET })));
    vi.stubGlobal("fetch", fetchMock);
    expect(await canReadSceneErrors(WALLET, "world", "my-world.dcl.eth")).toBe(true);
    expect(await canReadSceneErrors(OTHER, "world", "my-world.dcl.eth")).toBe(false);
    expect(await canReadSceneErrors(WALLET, "world", "../other")).toBe(false);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
