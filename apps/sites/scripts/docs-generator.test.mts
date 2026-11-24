import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, expect, it } from "vitest";
import { updateCommsDocs } from "./docs-comms-prose.mts";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "docs-generator-"));
const source = path.join(root, "source");
const output = path.join(root, "output");
const fallback = "creator/images/wearables-and-emotes/creating-wearables/19_skin_uv.webp";
const write = (base: string, rel: string, data: string | Buffer) => {
  const dest = path.join(base, rel);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.writeFileSync(dest, data);
};
const run = (check = false) => spawnSync(process.execPath, ["scripts/gen-docs-content.mts", ...(check ? ["--check"] : [])], {
  cwd: process.cwd(), env: { ...process.env, SRC_DOCS: source, OUT: output }, encoding: "utf8",
});
afterAll(() => fs.rmSync(root, { recursive: true, force: true }));

it("repairs upstream content, validates links and pinned media, and detects drift", () => {
  write(source, "player/README.md", "# Welcome\n\n[Account](https://docs.decentraland.org/faqs/my-account)\n");
  write(source, "player/faqs/my-account.md", "# Account\n");
  const revisedPage = "player/decentraland-in-world/overview.md";
  const originalOverview = "# Overview\r\n\r\nExplore Genesis City and it's archipelago of off-map Worlds.\r\n";
  write(source, revisedPage, originalOverview);
  write(source, "creator/README.md", '# Creator\n\n![\x01](images/pic.webp)\n\n\\\x01src="../images/wearables-and-emotes/creating-wearables/19\\_skin\\_uv.png"width="600"/>\n\n![Revocations Committee](https://github.com/lordlikedao/documentation/blob/main/static/images/DAO%20Organizational%20Chart/Revocation.png?raw=true)\n');
  write(source, "creator/images/pic.webp", "image");
  write(source, "player/images/favicon.ico", "icon");
  write(source, "player/images/DAO Organizational Chart/Revocation.webp", "chart");
  write(output, fallback, fs.readFileSync(path.join(process.cwd(), "content/docs", fallback)));
  execFileSync("git", ["init", "-q", source]);
  execFileSync("git", ["-C", source, "add", "."]);
  execFileSync("git", ["-C", source, "-c", "user.name=Docs test", "-c", "user.email=docs-test@example.invalid", "commit", "-qm", "fixture"]);
  const result = run();
  expect(result.status, result.stderr).toBe(0);
  const player = fs.readFileSync(path.join(output, "player/index.md"), "utf8");
  const creator = fs.readFileSync(path.join(output, "creator/index.md"), "utf8");
  expect(player).toContain("/docs/player/faqs/my-account/");
  expect(creator).toContain(`![Avatar skin UV map](/docs/${fallback})`);
  expect(creator).toContain("/docs/player/images/DAO%20Organizational%20Chart/Revocation.webp");
  expect(creator).not.toContain("\x01");
  const overview = fs.readFileSync(path.join(output, "player/decentraland-in-world/overview/index.md"), "utf8");
  expect(overview).toContain("and its off-map Worlds");
  expect(run(true).status).toBe(0);

  write(source, revisedPage, "# Overview\n\nExplore the city and nearby worlds.\n");
  for (const check of [false, true]) {
    const stale = run(check);
    expect(stale.status).toBe(1);
    expect(stale.stderr).toContain(`comms prose rule did not match in ${revisedPage}`);
  }
  write(source, revisedPage, originalOverview);

  fs.unlinkSync(path.join(output, fallback));
  expect(run(true).stderr).toContain("missing or altered pinned image");
  write(source, "player/README.md", "# Welcome\n\n[broken](/missing.md)\n");
  expect(run(true).stderr).toContain("broken local reference");
}, 30000);

it("preserves upstream additions around the reviewed main realm paragraphs", () => {
  const added = "## Realm availability\n\nCustom realm configuration can vary.";
  const input = [
    "Decentraland has as many realms as there are Catalyst nodes.",
    '<img src="/docs/contributor/.gitbook/assets/main-realm.webp" width="800"/>',
    "The [realm provider](https://github.com/decentraland/realm-provider) exposes realm information.",
    "The MAIN communications service is accessed through a WebSocket connector.",
    "[NATS](https://nats.io/) functions as a broadcasting system.",
    "This new paragraph is independent of the old service description.",
    "The Archipelago stats service shares the same API implementation.",
    "The [LiveKit](/docs/contributor/communications/transport-types/livekit/) Cloud platform handles media.",
    "The fundamental concept derived from the MAIN realm is shared communications.",
    added,
  ].join("\n\n");
  const revised = updateCommsDocs("contributor/communications/main-realm.md", input);
  expect(revised).toContain("Pulse handles player movement and clustering");
  expect(revised).toContain("This new paragraph is independent of the old service description.");
  expect(revised).toContain(added);
});
