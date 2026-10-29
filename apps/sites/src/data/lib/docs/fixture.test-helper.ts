import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import type { DocsManifest } from "./types";

const WEBP = Buffer.from("524946460400000057454250", "hex");

/** A two-space docs tree in a temp dir, shaped like content/docs; returns its path. */
export function writeDocsFixture(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "docs-fixture-"));
  const write = (rel: string, body: string | Buffer) => {
    fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
    fs.writeFileSync(path.join(dir, rel), body);
  };
  const manifest: DocsManifest = {
    generator: "fixture",
    source: { repo: "decentraland/docs", commit: "0", committedAt: "2026-09-01T00:00:00Z" },
    spaces: [
      { id: "player", title: "Player", blurb: "", icon: "home", header: true },
      { id: "creator", title: "Creator", blurb: "", icon: "laptop", header: true },
    ],
    nav: {
      player: { children: [{ title: "Welcome", url: "/docs/player/", children: [] }, { title: "FAQ", url: "/docs/player/faq/", children: [] }] },
      creator: { children: [{ type: "section", title: "Scenes", children: [
        { title: "Intro", url: "/docs/creator/intro/", children: [{ title: "Deep", url: "/docs/creator/intro/deep/", children: [] }] },
      ] }] },
    },
    pages: [
      { url: "/docs/", file: "index.md", title: "Welcome", description: "Start here", icon: "hand-wave", space: "player", source: "player/README.md", updatedAt: "2026-09-01T00:00:00Z", navUrl: "/docs/player/" },
      { url: "/docs/player/", file: "player/index.md", title: "Welcome", description: "Start here", icon: "hand-wave", space: "player", source: "player/README.md", updatedAt: null },
      { url: "/docs/player/faq/", file: "player/faq/index.md", title: "FAQ", description: null, icon: null, space: "player", source: "player/faq.md", updatedAt: null, tokens: 42 },
      { url: "/docs/creator/intro/", file: "creator/intro/index.md", title: "Intro", description: null, icon: null, space: "creator", source: "creator/intro.md", updatedAt: null },
      { url: "/docs/creator/intro/deep/", file: "creator/intro/deep/index.md", title: "Deep", description: null, icon: null, space: "creator", source: "creator/intro/deep.md", updatedAt: null },
    ],
  };
  const welcome = "> Start here\n\n# Welcome\n\nHello.\n\n## First steps\n\n> [!WARNING]\n> Mind the gap.\n";
  write("manifest.json", JSON.stringify(manifest));
  write("index.md", welcome);
  write("player/index.md", welcome);
  write("player/faq/index.md", "# FAQ\n\n## Why?\n\nBecause.\n\n### How?\n\nLike this.\n");
  write("creator/intro/index.md", "# Intro\n\n![shot](/docs/.gitbook/assets/shot.webp)\n");
  write("creator/intro/deep/index.md", "# Deep\n");
  write(".gitbook/assets/shot.webp", WEBP);
  write("_media.json", JSON.stringify({
    "/docs/.gitbook/assets/shot.webp": { source: ".gitbook/assets/shot.png", encoder: "fixture", key: "k", width: 1600, height: 800, bytes: 12, sourceBytes: 900000 },
  }));
  write("search-index.json", "[]");
  return dir;
}
