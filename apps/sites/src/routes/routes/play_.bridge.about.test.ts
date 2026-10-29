import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { loader } from "./play_.bridge.about";

let directory: string;
const hash = "bafkreiexample";
const realm = { comms: { fixedAdapter: "offline:offline" }, configurations: {
  realmName: "BevyExplorerUI", scenesUrn: [`urn:decentraland:entity:${hash}?=&baseUrl=/bridge-scene/static/`],
} };
async function save(value: unknown) {
  await writeFile(join(directory, "bridge-scene/static/BevyExplorerUI/about"), JSON.stringify(value));
}
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), "play-realm-"));
  vi.stubEnv("PLAY_WEB_DIR", directory);
  await mkdir(join(directory, "bridge-scene/static/BevyExplorerUI"), { recursive: true });
  await save(realm);
});
afterEach(async () => {
  vi.unstubAllEnvs();
  await rm(directory, { recursive: true });
});

it("serves the compiled bridge at the requesting deployment's absolute Sites path", async () => {
  for (const origin of ["https://sites.test", "http://localhost:5212"]) {
    const response = await loader({ request: new Request(`${origin}/play/bridge/about`, {
      headers: { "X-Forwarded-Host": "untrusted.test" },
    }) });
    const about = await response.json();
    expect(response.headers.get("Cache-Control")).toBe("no-cache");
    expect(about.comms).toEqual(realm.comms);
    expect(about.configurations.realmName).toBe("BevyExplorerUI");
    expect(about.configurations.scenesUrn).toEqual([
      `urn:decentraland:entity:${hash}?=&baseUrl=${origin}/play/bridge-scene/static/`,
    ]);
  }
});

it("rejects missing, foreign or malformed bridge content instead of redirecting the engine", async () => {
  for (const urn of [undefined, "urn:decentraland:entity:bafexample?=&baseUrl=https://untrusted.test/", "bad"]) {
    await save({ configurations: { scenesUrn: urn == null ? [] : [urn] } });
    await expect(loader({ request: new Request("https://sites.test/play/bridge/about") }))
      .rejects.toMatchObject({ status: 503 });
  }
});

it("uses the gateway's resolved TLS scheme across the private HTTP hop", async () => {
  const response = await loader({ request: new Request("http://sites.test/play/bridge/about", {
    headers: { "X-Forwarded-Proto": "https", "X-Forwarded-Host": "untrusted.test" },
  }) });
  const about = await response.json();
  expect(about.configurations.scenesUrn[0]).toContain("baseUrl=https://sites.test/play/bridge-scene/static/");
});
