// Builds public/errors/<status>.html: self-contained branded error pages that nginx serves for errors
// it generates itself (the app is down or timed out, rate limits, `return 404`, oversized bodies). The
// app's own 4xx/5xx render StatusPage (library/ui/apps/sites/status), whose copy, CSS Module and art
// these pages share. 404 is the app's own NotFoundPage (src/features/not-found), rendered from its
// component with the CSS Modules its markup uses, the shared Inter faces (library/ui/foundations/typography)
// for the scripts its text uses, and AnimatedBackground's animation (animate.ts, bundled
// into an inline script) over the still image it renders on the server.
//
// Every page inlines its CSS, the --dcl-* tokens that CSS reads (from library/ui/foundations), the logo,
// the art, fonts and scripts (as data: URIs or inline), so it renders even when nothing behind nginx is reachable.
//
// The output is not committed: scripts/sync-ui3-public.mts runs this after it fills public/ (predev,
// prebuild, the Nix sites package), so every build renders the pages from the current sources.
//
//   node scripts/gen-error-pages.mts [--check]
//   ERROR_PAGES_OUT=<dir> writes elsewhere than public/errors.
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { createElement, type ComponentType } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { build, createServer, type ModuleNode, type Rollup } from "vite";

const sites = fileURLToPath(new URL("../", import.meta.url));
const oneUi = fileURLToPath(new URL("../../../library/ui/", import.meta.url));
const webCore = fileURLToPath(new URL("../../../library/web-core/", import.meta.url));
const status = join(oneUi, "apps/sites/status");
const artDir = join(oneUi, "apps/sites/assets/errors");
const notFoundEntry = join(sites, "src/features/not-found/NotFoundPage.tsx");
const foundations = join(oneUi, "foundations");
const animatedBackground = join(oneUi, "components/AnimatedBackground");
// The shared Inter faces (variable Inter, one face per script spanning every weight) and their files.
const interCss = join(foundations, "typography/inter.css");
const outDir = process.env.ERROR_PAGES_OUT || join(sites, "public/errors");
const check = process.argv.includes("--check");

// The sources are TypeScript, a CSS Module and TSX: Vite (which every Sites install carries) loads
// them, so the class names and the compiled CSS are the ones the module produces.
const vite = await createServer({
  configFile: false,
  root: sites,
  logLevel: "silent",
  appType: "custom",
  server: { middlewareMode: true, hmr: false, watch: null },
  // The @dcl/one-ui and @web-core aliases of vite.config.ts, which NotFoundPage and the copy import through.
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: [
      { find: /^@web-core\//, replacement: webCore },
      { find: /^@dcl\/one-ui$/, replacement: join(oneUi, "index.ts") },
      { find: /^@dcl\/one-ui\/text-styles\.module\.css$/, replacement: join(oneUi, "foundations/typography/text-styles.module.css") },
      { find: /^@dcl\/one-ui\//, replacement: oneUi },
    ],
  },
  optimizeDeps: { noDiscovery: true, include: [] },
});
let copyModule: typeof import("@web-core/status-copy");
let styles: Record<string, string>;
let moduleCss: string;
// Typed by the one icon it reads: the entry's own types pull in its CSS and media imports, which this config does not declare.
let ui: { GeneralDecentraland: ComponentType<Record<string, unknown>> };
let notFoundMarkup: string;
let notFoundCss: string;
let backgroundStyles: Record<string, string>;
try {
  copyModule = await vite.ssrLoadModule("@web-core/status-copy") as typeof copyModule;
  styles = (await vite.ssrLoadModule(join(status, "StatusPage.module.css"))).default as Record<string, string>;
  moduleCss = (await vite.ssrLoadModule(`${join(status, "StatusPage.module.css")}?inline`)).default as string;
  ui = await vite.ssrLoadModule("@dcl/one-ui") as typeof ui;
  const NotFoundPage = (await vite.ssrLoadModule(notFoundEntry)).default as ComponentType;
  // React hoists a preload for each <img>; the page inlines the image, so the preload would only repeat its bytes.
  notFoundMarkup = renderToStaticMarkup(createElement(MemoryRouter, null, createElement(NotFoundPage)))
    .replace(/<link rel="preload" as="image"[^>]*>/g, "");
  notFoundCss = await usedModuleCss(vite, notFoundEntry, notFoundMarkup);
  backgroundStyles = (await vite.ssrLoadModule(join(animatedBackground, "AnimatedBackground.module.css"))).default as Record<string, string>;
} finally {
  await vite.close();
}
/**
 * The CSS Modules the entry imports (dependencies first, the order a build emits them) whose classes the
 * markup uses: `@dcl/one-ui`'s index brings every component's module, and only the used ones belong here.
 */
async function usedModuleCss(vite: Awaited<ReturnType<typeof createServer>>, entry: string, markup: string): Promise<string> {
  const used = new Set([...markup.matchAll(/class="([^"]*)"/g)].flatMap((m) => m[1]!.split(/\s+/)));
  const files: string[] = [];
  const seen = new Set<ModuleNode>();
  const walk = (node: ModuleNode | undefined) => {
    if (!node || seen.has(node)) return;
    seen.add(node);
    for (const dep of node.ssrImportedModules) walk(dep);
    if (node.file?.endsWith(".module.css")) files.push(node.file);
  };
  walk(await vite.moduleGraph.getModuleByUrl(entry, true));
  const sheets: string[] = [];
  for (const file of files) {
    const classes = Object.values((await vite.ssrLoadModule(file)).default as Record<string, string>);
    if (classes.some((name) => used.has(name))) sheets.push((await vite.ssrLoadModule(`${file}?inline`)).default as string);
  }
  if (!sheets.length) throw new Error(`gen-error-pages: no CSS Module of ${entry} styles its markup`);
  return sheets.join("\n");
}

const { ART_FILES, LOGOMARK_VIEW_BOX, STATIC_ERROR_STATUSES, isRetryable, statusCopy } = copyModule;

const MEDIA_BLOCK = /@media[^{]*\{(?:[^{}]*\{[^{}]*\})*\s*\}/g;

/** The --dcl-* values the foundation stylesheets declare outside media queries, and the names a media query overrides. */
function foundationTokens(): { values: Map<string, string>; responsive: Set<string> } {
  const values = new Map<string, string>();
  const responsive = new Set<string>();
  const sheets = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
      e.isDirectory() ? sheets(join(dir, e.name)) : e.name.endsWith(".css") && !e.name.endsWith(".module.css") ? [join(dir, e.name)] : [],
    );
  for (const file of sheets(foundations)) {
    const css = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    for (const block of css.matchAll(MEDIA_BLOCK)) {
      for (const m of block[0].matchAll(/(--dcl-[\w-]+)\s*:/g)) responsive.add(m[1]!);
    }
    for (const m of css.replace(MEDIA_BLOCK, "").matchAll(/(--dcl-[\w-]+)\s*:\s*([^;]+);/g)) {
      const name = m[1]!;
      const value = m[2]!.trim();
      if (values.has(name) && values.get(name) !== value) throw new Error(`gen-error-pages: ${name} has two values in library/ui/foundations`);
      values.set(name, value);
    }
  }
  return { values, responsive };
}

/** `:root{...}` with every token the CSS reads, and the tokens those read, sorted by name. */
function tokenBlock(css: string): string {
  const { values, responsive } = foundationTokens();
  const needed = new Map<string, string>();
  const visit = (text: string) => {
    for (const m of text.matchAll(/var\(\s*(--dcl-[\w-]+)/g)) {
      const name = m[1]!;
      if (needed.has(name)) continue;
      const value = values.get(name);
      if (value === undefined) throw new Error(`gen-error-pages: ${name} is not defined in library/ui/foundations`);
      if (responsive.has(name)) throw new Error(`gen-error-pages: ${name} changes per breakpoint; the static pages carry no media queries`);
      needed.set(name, value);
      visit(value);
    }
  };
  visit(css);
  return `:root{${[...needed].sort(([a], [b]) => a.localeCompare(b)).map(([name, value]) => `${name}:${value}`).join(";")}}`;
}

const escape = (text: string) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const cls = (...names: string[]) =>
  names.map((name) => {
    const scoped = styles[name];
    if (!scoped) throw new Error(`gen-error-pages: StatusPage.module.css has no .${name}`);
    return scoped;
  }).join(" ");

const LFS_POINTER = "version https://git-lfs.github.com/spec/v1";
const artData = new Map<string, string>();
const artUri = (file: string) => webpUri(join(artDir, file));
const webpUri = (path: string) => {
  const file = relative(join(oneUi, "../.."), path);
  if (!artData.has(path)) {
    if (!path.endsWith(".webp")) throw new Error(`gen-error-pages: ${file} is not a webp image`);
    const bytes = readFileSync(path);
    if (bytes.subarray(0, LFS_POINTER.length).toString("latin1") === LFS_POINTER) {
      throw new Error(
        `gen-error-pages: ${file} is a Git LFS pointer, not the image. From the repository root run: git lfs pull --include="${file.startsWith("library/ui/assets/") ? "library/ui/assets" : "library/ui/apps/sites/assets"}/**"`,
      );
    }
    if (bytes.subarray(0, 4).toString("latin1") !== "RIFF" || bytes.subarray(8, 12).toString("latin1") !== "WEBP") {
      throw new Error(`gen-error-pages: ${file} is not a webp image`);
    }
    artData.set(path, `data:image/webp;base64,${bytes.toString("base64")}`);
  }
  return artData.get(path)!;
};
/** Vite serves a file outside the Sites root at /@fs/<absolute path>, one inside it at its root-relative path. */
const assetUri = (url: string) => {
  const path = decodeURIComponent(url);
  return webpUri(path.startsWith("/@fs/") ? path.slice("/@fs".length) : join(sites, path));
};

const woff2Uri = (path: string) => {
  const file = relative(join(oneUi, "../.."), path);
  const bytes = readFileSync(path);
  if (bytes.subarray(0, LFS_POINTER.length).toString("latin1") === LFS_POINTER) {
    throw new Error(
      `gen-error-pages: ${file} is a Git LFS pointer, not the font. From the repository root run: git lfs pull --include="library/ui/foundations/typography/fonts/**"`,
    );
  }
  if (!path.endsWith(".woff2") || bytes.subarray(0, 4).toString("latin1") !== "wOF2") throw new Error(`gen-error-pages: ${file} is not a woff2 font`);
  return `data:font/woff2;base64,${bytes.toString("base64")}`;
};

/** Whether the CSS unicode-range `range` holds any of `points`. */
const inRange = (range: string, points: Set<number>) =>
  range.split(",").some((part) => {
    const [lo, hi = lo] = part.trim().replace(/^U\+/i, "").split("-").map((h) => parseInt(h!, 16));
    return [...points].some((p) => p >= lo! && p <= hi!);
  });

/**
 * The shared Inter @font-face rules (library/ui/foundations/typography/inter.css) for the scripts the markup's text
 * uses, with their files inlined. A code point no selected face covers, or a weight `css` sets outside a face's
 * range, is an error: it would render in the fallback font or a synthesized weight.
 */
function interFaces(css: string, markup: string): string {
  const faces = [...readFileSync(interCss, "utf8").replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/@font-face\s*\{([^}]*)\}/g)].map((m) => {
    const body = m[1]!.trim();
    const prop = (name: string) => new RegExp(`${name}\\s*:\\s*([^;]+)`).exec(body)?.[1]!.trim() ?? "";
    const [lo, hi = lo] = prop("font-weight").split(/\s+/).map(Number);
    return { body, weights: [lo!, hi!], range: prop("unicode-range"), url: /url\(\s*["']?(\.\/fonts\/[^"')]+)["']?\s*\)/.exec(body)?.[1] };
  });
  if (!faces.length || faces.some((face) => !face.url || !face.weights[0] || !face.range)) throw new Error(`gen-error-pages: cannot read the faces of ${relative(sites, interCss)}`);
  const text = markup.replace(/<[^>]*>/g, "")
    .replace(/&#x([\da-f]+);/gi, (_, h: string) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/&#(\d+);/g, (_, d: string) => String.fromCodePoint(Number(d)))
    .replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
  const points = new Set([...text.replace(/\s+/g, " ")].map((c) => c.codePointAt(0)!));
  const used = faces.filter((face) => inRange(face.range, points));
  const missing = [...points].filter((p) => !used.some((face) => inRange(face.range, new Set([p]))));
  if (missing.length) throw new Error(`gen-error-pages: no Inter face covers ${missing.map((p) => `U+${p.toString(16).toUpperCase()}`).join(", ")}`);
  for (const weight of new Set([400, ...[...css.matchAll(/font-weight\s*:\s*(\d+)/g)].map((m) => Number(m[1]))])) {
    const outside = used.filter((face) => weight < face.weights[0] || weight > face.weights[1]);
    if (outside.length) throw new Error(`gen-error-pages: font-weight ${weight} is outside the Inter face of ${outside[0]!.url}`);
  }
  return used.map((face) => `@font-face{${face.body.replace(face.url!, woff2Uri(join(interCss, "..", face.url!))).replace(/\s*([:;,])\s*/g, "$1")}}`).join("");
}

/**
 * AnimatedBackground's animation (animate.ts), bundled with its shaders and the overlay texture inlined, started on
 * the root AnimatedBackground rendered into the 404 markup.
 */
async function backgroundScript(markup: string): Promise<string> {
  const root = backgroundStyles.root;
  const canvas = backgroundStyles.canvas;
  if (!root || !canvas || !markup.includes(`class="${root}"`)) throw new Error("gen-error-pages: the 404 markup has no AnimatedBackground root");
  const entry = "\0static-404-background";
  const output = await build({
    configFile: false,
    root: sites,
    logLevel: "silent",
    publicDir: false,
    plugins: [{
      name: "static-404-background",
      enforce: "pre",
      resolveId: (id) => (id === entry ? entry : null),
      load(id) {
        if (id === entry) {
          return `import { animateBackground } from ${JSON.stringify(join(animatedBackground, "animate.ts"))};
const root = document.querySelector(${JSON.stringify(`.${root}`)});
if (root) animateBackground(root, ${JSON.stringify(canvas)});`;
        }
        // Through the same checks as the art: real webp bytes, not a Git LFS pointer.
        if (id.endsWith(".webp")) return `export default ${JSON.stringify(webpUri(id))};`;
        return null;
      },
    }],
    build: { write: false, minify: true, modulePreload: false, copyPublicDir: false, rollupOptions: { input: entry, output: { format: "iife" } } },
  }) as Rollup.RollupOutput | Rollup.RollupOutput[];
  const files = (Array.isArray(output) ? output : [output]).flatMap((o) => o.output);
  const chunk = files.find((file): file is Rollup.OutputChunk => file.type === "chunk");
  if (!chunk || files.length !== 1) throw new Error(`gen-error-pages: the background bundle is ${files.map((f) => f.fileName).join(", ")}, not one script`);
  return chunk.code.trim().replace(/<\/script/gi, "<\\/script");
}

const minify = (text: string) => text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\s*\n\s*/g, "");
const SHELL_CSS = "html,body{margin:0;background:#0d0c11}";
const css = minify(moduleCss);
const style = `${tokenBlock(css)}${SHELL_CSS}${css}`;
const notFoundStyleCss = minify(notFoundCss).replace(/url\(\s*(['"]?)(\/[^'")]+)\1\s*\)/g, (_, _q, url: string) => `url(${assetUri(url)})`);
const notFoundStyle = `${interFaces(notFoundStyleCss, notFoundMarkup)}${tokenBlock(notFoundStyleCss)}${SHELL_CSS}${notFoundStyleCss}`;
const notFoundBody = `${notFoundMarkup.replace(/ src="(\/[^"]+)"/g, (_, url: string) => ` src="${assetUri(url)}"`)}\n<script>${await backgroundScript(notFoundMarkup)}</script>`;
const logo = renderToStaticMarkup(
  createElement(ui.GeneralDecentraland, { className: styles.logoMark, size: 28, viewBox: LOGOMARK_VIEW_BOX, "aria-hidden": "true" }),
);

function head(code: number, source: string, css: string): string {
  return `<!doctype html>
<!-- Generated by apps/sites/scripts/gen-error-pages.mts from ${source}. Do not edit. -->
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>${code} · ${escape(statusCopy(code).eyebrow)} · Decentraland</title>
<style>${css}</style>
</head>`;
}

function page(code: number): string {
  if (code === 404) return `${head(code, "apps/sites/src/features/not-found", notFoundStyle)}\n<body>\n${notFoundBody}\n</body>\n</html>\n`;
  const copy = statusCopy(code);
  const art = ART_FILES[copy.art];
  const second = isRetryable(code)
    ? `<button type="button" class="${cls("button", "secondary")}" onclick="location.reload()">Try again</button>`
    : `<button type="button" class="${cls("button", "secondary")}" onclick="history.back()">Go back</button>`;
  return `${head(code, "library/ui/apps/sites/status", style)}
<body>
<main class="${cls("page")}" aria-labelledby="sp-title" data-status="${code}">
<div class="${cls("card")}">
<a class="${cls("logo")}" href="/">${logo}<span>Decentraland</span></a>
<img class="${cls("art")}" src="${artUri(art.file)}" width="${art.width}" height="${art.height}" alt="">
<p class="${cls("code")}">${code} · ${escape(copy.eyebrow)}</p>
<h1 id="sp-title" class="${cls("title")}">${escape(copy.title)}</h1>
<p class="${cls("message")}">${escape(copy.message)}</p>
<nav class="${cls("actions")}" aria-label="Error page navigation"><a class="${cls("button", "primary")}" href="/">Go to homepage</a>${second}</nav>
</div>
</main>
</body>
</html>
`;
}

mkdirSync(outDir, { recursive: true });
const expected = new Map(STATIC_ERROR_STATUSES.map((code) => [`${code}.html`, page(code)]));
const stale: string[] = [];
for (const [name, html] of expected) {
  const file = join(outDir, name);
  let current = "";
  try { current = readFileSync(file, "utf8"); } catch {}
  if (current === html) continue;
  if (check) stale.push(name);
  else writeFileSync(file, html);
}
for (const name of readdirSync(outDir)) {
  if (!expected.has(name)) stale.push(`${name} (unexpected)`);
}
if (check && stale.length) {
  console.error(`[gen:error-pages] out of date: ${stale.join(", ")}\n  run: node scripts/gen-error-pages.mts`);
  process.exit(1);
}
console.log(check ? "[gen:error-pages] up to date" : `[gen:error-pages] wrote ${expected.size} pages to ${outDir}`);
