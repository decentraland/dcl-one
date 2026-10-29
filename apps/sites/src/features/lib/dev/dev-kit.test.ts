import { afterEach, beforeEach, expect, it, vi } from "vitest";

const signing = vi.hoisted(() => ({
  loads: 0,
  signIn: vi.fn(async () => ({ signer: "burner" })),
  generate: vi.fn(() => `0x${"1".repeat(64)}`),
}));

vi.mock("@/data/lib/auth/dev-identity", () => {
  signing.loads += 1;
  return { signInWithPrivateKey: signing.signIn };
});
vi.mock("viem/accounts", () => ({ generatePrivateKey: signing.generate }));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  signing.loads = 0;
  vi.stubGlobal("window", { localStorage: { setItem: vi.fn() } });
  vi.spyOn(console, "info").mockImplementation(() => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("keeps signing code unloaded until the console burner helper is used", async () => {
  const { installDevKit } = await import("./dev-kit");
  installDevKit();
  const kit = (window as unknown as { __DCL_DEV__: import("./dev-kit").DclDevKit }).__DCL_DEV__;
  expect(signing.loads).toBe(0);
  expect(signing.generate).not.toHaveBeenCalled();

  const key = `0x${"2".repeat(64)}` as const;
  await expect(kit.signInBurner(key)).resolves.toEqual({ signer: "burner" });
  expect(signing.loads).toBe(1);
  expect(signing.generate).not.toHaveBeenCalled();
  expect(signing.signIn).toHaveBeenCalledWith(key, { allowNonDev: true });
  expect(window.localStorage.setItem).toHaveBeenCalledWith("dcl:auth:dev-signer-pk:v1", key);

  await kit.signInBurner();
  expect(signing.generate).toHaveBeenCalledOnce();
  expect(signing.signIn).toHaveBeenLastCalledWith(`0x${"1".repeat(64)}`, { allowNonDev: true });
});
