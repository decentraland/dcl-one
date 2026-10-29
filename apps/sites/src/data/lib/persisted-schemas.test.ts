import { describe, expect, test } from "vitest";

import {
  AuthIdentitySchema,
  DevSignerKeySchema,
  SimCollectionItemsStoreSchema,
  ThirdwebSessionSchema,
} from "./persisted-schemas";

type Case = [name: string, value: unknown, schemaAccepts: boolean];

function table(
  label: string,
  schema: { safeParse(v: unknown): { success: boolean } },
  guard: (v: unknown) => boolean,
  cases: Case[],
): void {
  test(`${label}: the schema accepts exactly what this build writes and every shape passes the legacy guard`, () => {
    const offenders: string[] = [];
    for (const [name, value, schemaAccepts] of cases) {
      if (schema.safeParse(value).success !== schemaAccepts) {
        offenders.push(`schema ${schemaAccepts ? "rejected" : "accepted"}: ${name}`);
      }
      if (!guard(value)) offenders.push(`guard rejected: ${name}`);
    }
    expect(offenders).toEqual([]);
  });
}

describe("auth identity", () => {
  const guard = (v: unknown) => {
    const p = v as {
      signer?: unknown;
      ephemeral?: { privateKey?: unknown };
      authChain?: unknown;
    };
    return Boolean(p?.signer && p?.ephemeral?.privateKey && Array.isArray(p.authChain));
  };
  const EPHEMERAL_KEY = `0x${"a1b2c3d4".repeat(8)}`;
  const identity = (over: Record<string, unknown> = {}) => ({
    signer: "0xabc",
    ephemeral: { address: "0xeee", privateKey: EPHEMERAL_KEY },
    expiration: "2099-01-01T00:00:00.000Z",
    authChain: [{ type: "SIGNER", payload: "0xabc", signature: "" }],
    ...over,
  });

  table("auth identity", AuthIdentitySchema, guard, [
    ["what this build writes", identity(), true],
    [
      "a private key stored without its 0x prefix",
      identity({ ephemeral: { address: "0xeee", privateKey: "deadbeef" } }),
      false,
    ],
    [
      "an identity written before it carried an expiration",
      identity({ expiration: undefined }),
      false,
    ],
    [
      "an auth link missing its signature",
      identity({ authChain: [{ type: "SIGNER", payload: "0xabc" }] }),
      false,
    ],
    [
      "an auth link type this build does not know",
      identity({ authChain: [{ type: "EIP1271", payload: "0xabc", signature: "0x1" }] }),
      false,
    ],
  ]);
});

describe("thirdweb session", () => {
  const guard = (v: unknown) => {
    const p = v as { token?: unknown; address?: unknown };
    return Boolean(p?.token && p?.address);
  };

  table("thirdweb session", ThirdwebSessionSchema, guard, [
    ["what this build writes", { token: "eyJ...", address: "0xabc" }, true],
    ["a token stored as its expiry epoch", { token: 1893456000, address: "0xabc" }, false],
    [
      "an address that became the whole account object",
      { token: "eyJ...", address: { value: "0xabc" } },
      false,
    ],
  ]);
});

describe("sim collection items", () => {
  const guard = (v: unknown) => Boolean(v && typeof v === "object");
  const entry = (files: unknown[]) => ({ c1: { ts: 1_700_000_000_000, files } });

  table("sim collection items", SimCollectionItemsStoreSchema, guard, [
    ["what this build writes", entry([{ name: "hat.glb", size: 12, fileType: "model/gltf" }]), true],
    ["a file size stored as a formatted string", entry([{ name: "hat.glb", size: "12 KB", fileType: "model/gltf" }]), false],
    ["a draft reduced to its file names", entry(["hat.glb"]), false],
  ]);
});

describe("dev signer key", () => {
  const guard = (v: unknown) => typeof v === "string" && v.startsWith("0x");
  const key = `0x${"a".repeat(64)}`;

  table("dev signer key", DevSignerKeySchema, guard, [
    ["a real burner key", key, true],
    ["a truncated key, which viem would throw on", "0xdeadbeef", false],
    ["an address stored where the key belongs", `0x${"b".repeat(40)}`, false],
  ]);
});

