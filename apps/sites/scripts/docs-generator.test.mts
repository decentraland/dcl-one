import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, expect, it } from "vitest";

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
  write(source, "creator/README.md", '# Creator\n\n![\x01](images/pic.webp)\n\n\\\x01src="../images/wearables-and-emotes/creating-wearables/19\\_skin\\_uv.png"width="600"/>\n\n![Revocations Committee](https://github.com/lordlikedao/documentation/blob/main/static/images/DAO%20Organizational%20Chart/Revocation.png?raw=true)\n');
  write(source, "creator/images/pic.webp", "image");
  write(source, "creator/sdk7/publishing/publishing-options.md", "# Publishing\n");
  write(source, "creator/3d-modeling/meshes.md", "# Meshes\n");
  write(source, "creator/.gitbook.yaml", "structure:\n  readme: README.md\n\nredirects:\n  worlds/cast: sdk7/publishing/publishing-options.md\n  3d-modeling/meshes: 3d-modeling/meshes.md\n  development-guide/sdk-101: sdk7/Publishing/Publishing-Options.md\n  gone: nowhere.md\n");
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
  expect(JSON.parse(fs.readFileSync(path.join(output, "manifest.json"), "utf8")).redirects).toEqual({
    "/docs/creator/development-guide/sdk-101/": "/docs/creator/sdk7/publishing/publishing-options/",
    "/docs/creator/worlds/cast/": "/docs/creator/sdk7/publishing/publishing-options/",
  });
  expect(run(true).status).toBe(0);
  fs.unlinkSync(path.join(output, fallback));
  expect(run(true).stderr).toContain("missing or altered pinned image");
  write(source, "player/README.md", "# Welcome\n\n[broken](/missing.md)\n");
  expect(run(true).stderr).toContain("broken local reference");
}, 30000);
