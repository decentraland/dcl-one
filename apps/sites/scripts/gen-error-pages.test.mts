import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, expect, it } from "vitest";

import { FRAGMENT_SHADER, VERTEX_SHADER } from "@dcl/one-ui";
import { ART_FILES, isRetryable, STATIC_ERROR_STATUSES, statusCopy } from "@web-core/status-copy";

const SITES = fileURLToPath(new URL("../", import.meta.url));
const SCRIPT = path.join(SITES, "scripts/gen-error-pages.mts");
const OUT = fs.mkdtempSync(path.join(os.tmpdir(), "gen-error-pages-"));
const run = (...args: string[]) =>
  spawnSync(process.execPath, [SCRIPT, ...args], { cwd: SITES, env: { ...process.env, ERROR_PAGES_OUT: OUT }, encoding: "utf8" });
const ASSETS = fileURLToPath(new URL("../../../library/ui/apps/sites/assets/", import.meta.url));
const SHARED_ASSETS = fileURLToPath(new URL("../../../library/ui/assets/", import.meta.url));
const FONTS = fileURLToPath(new URL("../../../library/ui/foundations/typography/fonts/", import.meta.url));
const REPOSITORY = fileURLToPath(new URL("../../../", import.meta.url));
const STATUS_PAGES = STATIC_ERROR_STATUSES.filter((s) => s !== 404);
const webpUri = (file: string, dir = ASSETS) => `data:image/webp;base64,${fs.readFileSync(path.join(dir, file)).toString("base64")}`;
const escape = (text: string) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

beforeAll(() => {
  const result = run();
  expect(result.status, result.stderr).toBe(0);
}, 60_000);
afterAll(() => fs.rmSync(OUT, { recursive: true, force: true }));

it("writes one page per static status and nothing else", () => {
  expect(fs.readdirSync(OUT).sort()).toEqual(STATIC_ERROR_STATUSES.map((s) => `${s}.html`).sort());
  expect(STATIC_ERROR_STATUSES).toEqual(expect.arrayContaining([404, 429, 500, 503]));
});

it("renders each page but 404 as the status page, with its copy, the home link and the right second action", () => {
  for (const status of STATUS_PAGES) {
    const html = fs.readFileSync(path.join(OUT, `${status}.html`), "utf8");
    const copy = statusCopy(status);
    expect(html, String(status)).toContain(`<title>${status} · ${escape(copy.eyebrow)} · Decentraland</title>`);
    expect(html, String(status)).toContain(escape(copy.title));
    expect(html, String(status)).toContain(escape(copy.message));
    expect(html, String(status)).toMatch(/<a [^>]*href="\/"[^>]*><svg[^>]*>.*?<\/svg><span>Decentraland<\/span><\/a>/s);
    expect(html, String(status)).toMatch(/<a [^>]*href="\/"[^>]*>Go to homepage<\/a>/);
    expect(html, String(status)).toContain(isRetryable(status) ? 'onclick="location.reload()">Try again' : 'onclick="history.back()">Go back');
  }
});

it("inlines the art as real webp bytes, so a Git LFS pointer cannot pass", () => {
  for (const status of STATIC_ERROR_STATUSES) {
    const html = fs.readFileSync(path.join(OUT, `${status}.html`), "utf8");
    const match = /<img [^>]*src="data:image\/webp;base64,([^"]+)"/.exec(html);
    expect(match, String(status)).not.toBeNull();
    const bytes = Buffer.from(match![1]!, "base64");
    expect(bytes.subarray(0, 4).toString("latin1"), String(status)).toBe("RIFF");
    expect(bytes.subarray(8, 12).toString("latin1"), String(status)).toBe("WEBP");
    expect(bytes.length, String(status)).toBeGreaterThan(1024);
  }
});

it("renders 404 as the Sites not-found page: its heading, copy, links, avatar art and the background's still image", () => {
  const html = fs.readFileSync(path.join(OUT, "404.html"), "utf8");
  expect(html).toContain('data-status="404"');
  expect(html).toMatch(/<p [^>]*aria-hidden="true">404<\/p>/);
  const heading = /<h1[^>]*>(.*?)<\/h1>/s.exec(html);
  expect(heading?.[1]?.replace(/<[^>]+>/g, "")).toBe("Oops! This page has left the world.");
  expect(html).toContain(
    "The link may be outdated, but Decentraland is still live. Find community events, DJ sets, creator-built games, and new experiences happening in-world.",
  );
  expect(html).toMatch(/<a [^>]*aria-label="Decentraland Home"[^>]*href="\/"[^>]*><svg[^>]*>.*?<\/svg><\/a>/s);
  expect(html).toMatch(/<a [^>]*href="\/whats-on"[^>]*>.*?See Events.*?<\/a>/s);
  expect(html).toContain(`src="${webpUri("errors/not-found-avatar.webp")}"`);
  const backgrounds = [...html.matchAll(/background-image:\s*url\(["']?([^"')]+)["']?\)/g)].map((m) => m[1]);
  expect(backgrounds).toContain(webpUri("custom-welcome-background.webp", SHARED_ASSETS));
  expect(html).not.toContain("Go to homepage");
});

it("embeds the shared Inter variable face for the latin text only, as real woff2 bytes", () => {
  const html = fs.readFileSync(path.join(OUT, "404.html"), "utf8");
  const faces = [...html.matchAll(/@font-face\s*\{([^}]*)\}/g)].map((m) => m[1]!);
  expect(faces).toHaveLength(1);
  const face = faces[0]!;
  expect(face).toMatch(/font-family:\s*['"]?Inter['"]?/);
  expect(face).toMatch(/font-weight:\s*100 900/);
  const data = /src:\s*url\(["']?data:font\/woff2;base64,([^"')]+)["']?\)/.exec(face);
  expect(data).not.toBeNull();
  const bytes = Buffer.from(data![1]!, "base64");
  expect(bytes.subarray(0, 4).toString("latin1")).toBe("wOF2");
  expect(bytes.equals(fs.readFileSync(path.join(FONTS, "inter-latin-wght-normal.woff2")))).toBe(true);
  for (const status of STATUS_PAGES) {
    expect(fs.readFileSync(path.join(OUT, `${status}.html`), "utf8"), String(status)).not.toContain("@font-face");
  }
});

it("runs the AnimatedBackground animation on the 404 from one inline script with its shaders and overlay texture", () => {
  const html = fs.readFileSync(path.join(OUT, "404.html"), "utf8");
  const scripts = [...html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g)];
  expect(scripts).toHaveLength(1);
  const [, attrs, code] = scripts[0]!;
  expect(attrs).not.toMatch(/src=/);
  for (const line of `${VERTEX_SHADER}\n${FRAGMENT_SHADER}`.split("\n").map((l) => l.trim()).filter(Boolean)) {
    expect(code, line).toContain(line);
  }
  expect(code).toContain("webgl");
  expect(code).toContain("prefers-reduced-motion: reduce");
  expect(code).toContain("webglcontextlost");
  expect(code).toContain("devicePixelRatio");
  expect(code).toContain(webpUri("logo-pattern.webp", SHARED_ASSETS));
  for (const status of STATUS_PAGES) {
    expect(fs.readFileSync(path.join(OUT, `${status}.html`), "utf8"), String(status)).not.toMatch(/<script[ >]/);
  }
});

/** Runs the check with `file` (under the repository) replaced by a Git LFS pointer in an override tree. */
function checkWithPointer(file: string) {
  const overrides = fs.mkdtempSync(path.join(os.tmpdir(), "gen-error-pages-assets-"));
  const stand = path.join(overrides, path.relative(REPOSITORY, file));
  fs.mkdirSync(path.dirname(stand), { recursive: true });
  fs.writeFileSync(stand, "version https://git-lfs.github.com/spec/v1\noid sha256:0000\nsize 1\n");
  try {
    return spawnSync(process.execPath, [SCRIPT, "--check"], {
      cwd: SITES,
      env: { ...process.env, ERROR_PAGES_OUT: OUT, ERROR_PAGES_ASSET_OVERRIDES: overrides },
      encoding: "utf8",
    });
  } finally {
    fs.rmSync(overrides, { recursive: true, force: true });
  }
}

it("fails on a Git LFS pointer in place of the art, naming the file and the command that fetches it, without touching the tracked file", () => {
  const art = path.join(ASSETS, "errors", ART_FILES[statusCopy(503).art].file);
  const bytes = fs.readFileSync(art);
  const result = checkWithPointer(art);
  expect(result.status).toBe(1);
  expect(result.stderr).toContain(`library/ui/apps/sites/assets/errors/${path.basename(art)} is a Git LFS pointer`);
  expect(result.stderr).toContain('git lfs pull --include="library/ui/apps/sites/assets/**"');
  expect(fs.readFileSync(art).equals(bytes)).toBe(true);
}, 60_000);

it("fails on a Git LFS pointer in place of the Inter file the 404 embeds, naming the file and the command that fetches it, without touching the tracked file", () => {
  const font = path.join(FONTS, "inter-latin-wght-normal.woff2");
  const bytes = fs.readFileSync(font);
  const result = checkWithPointer(font);
  expect(result.status).toBe(1);
  expect(result.stderr).toContain("library/ui/foundations/typography/fonts/inter-latin-wght-normal.woff2 is a Git LFS pointer");
  expect(result.stderr).toContain('git lfs pull --include="library/ui/foundations/typography/fonts/**"');
  expect(fs.readFileSync(font).equals(bytes)).toBe(true);
}, 60_000);

it("reads the tracked files when the override tree does not hold them", () => {
  const overrides = fs.mkdtempSync(path.join(os.tmpdir(), "gen-error-pages-assets-"));
  try {
    const result = spawnSync(process.execPath, [SCRIPT, "--check"], {
      cwd: SITES,
      env: { ...process.env, ERROR_PAGES_OUT: OUT, ERROR_PAGES_ASSET_OVERRIDES: overrides },
      encoding: "utf8",
    });
    expect(result.status, result.stderr).toBe(0);
  } finally {
    fs.rmSync(overrides, { recursive: true, force: true });
  }
}, 60_000);

it("is self-contained: no external stylesheet, script, font or image, and every custom property it reads is defined", () => {
  for (const status of STATIC_ERROR_STATUSES) {
    const html = fs.readFileSync(path.join(OUT, `${status}.html`), "utf8");
    expect(html, String(status)).not.toMatch(/<link [^>]*rel="stylesheet"|<script [^>]*src=|src="(?!data:)|url\((?!["']?(?:data:|#))/);
    const defined = new Set([...html.matchAll(/(--[\w-]+)\s*:/g)].map((m) => m[1]));
    const used = [...new Set([...html.matchAll(/var\((--[\w-]+)/g)].map((m) => m[1]))];
    expect(used.filter((name) => !defined.has(name)), String(status)).toEqual([]);
  }
});

it("reports the pages up to date right after writing them, and a stale or unexpected page as out of date", () => {
  expect(run("--check").status).toBe(0);
  const page = path.join(OUT, "503.html");
  const html = fs.readFileSync(page, "utf8");
  fs.writeFileSync(page, html.replace("We'll be right back", "Stale"));
  fs.writeFileSync(path.join(OUT, "999.html"), "");
  try {
    const result = run("--check");
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("503.html, 999.html (unexpected)");
  } finally {
    fs.writeFileSync(page, html);
    fs.rmSync(path.join(OUT, "999.html"));
  }
}, 60_000);

it("fails on a Git LFS pointer in place of the shared background, naming the file and the command that fetches it", () => {
  const image = path.join(SHARED_ASSETS, "custom-welcome-background.webp");
  const bytes = fs.readFileSync(image);
  fs.writeFileSync(image, "version https://git-lfs.github.com/spec/v1\noid sha256:0000\nsize 1\n");
  try {
    const result = run("--check");
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("library/ui/assets/custom-welcome-background.webp is a Git LFS pointer");
    expect(result.stderr).toContain('git lfs pull --include="library/ui/assets/**"');
  } finally {
    fs.writeFileSync(image, bytes);
  }
  expect(fs.readFileSync(image).equals(bytes)).toBe(true);
}, 60_000);
