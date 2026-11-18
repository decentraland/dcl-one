// Shared helpers for the agent config files the Hub writes into a scene folder (Claude Code's
// `.mcp.json`, Cursor's `.cursor/mcp.json`, Gemini's `.gemini/settings.json`) and for the
// `.gitignore` rules that keep machine-local files out of a creator's repo; also the `.dclignore`
// rules that keep editor and agent files out of a deployed scene and the npm overrides a new
// scene starts with.
import fs from 'fs';
import path from 'path';

import { DEFAULT_DCLIGNORE, parseDclignore } from './optimizer/dclignore';

export function readJsonObject(file: string): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed))
      return parsed as Record<string, unknown>;
  } catch {
    /* no file yet, or invalid JSON — start fresh */
  }
  return {};
}

function serversOf(config: Record<string, unknown>): Record<string, unknown> {
  return config.mcpServers !== null && typeof config.mcpServers === 'object'
    ? (config.mcpServers as Record<string, unknown>)
    : {};
}

// Make sure `<dir>/.gitignore` covers `entry` (as `entry`, `/entry` or `entry/`, or any of
// `coveredBy`). Idempotent: appends only when nothing already ignores it, and never rewrites
// what the creator wrote. `comment` goes on its own line above the rule.
export function ensureGitignored(
  dir: string,
  entry: string,
  opts: { coveredBy?: string[]; comment?: string } = {},
): void {
  const gitignore = path.join(dir, '.gitignore');
  let content = '';
  try {
    content = fs.readFileSync(gitignore, 'utf8');
  } catch {
    /* no .gitignore yet — created below */
  }
  const patterns = [entry, `/${entry}`, `${entry}/`, ...(opts.coveredBy ?? [])];
  const covered = content.split(/\r?\n/).some(l => patterns.includes(l.trim()));
  if (covered) return;
  const head = content === '' || content.endsWith('\n') ? content : `${content}\n`;
  const comment = opts.comment !== undefined ? `\n# ${opts.comment}\n` : '';
  fs.writeFileSync(gitignore, `${head}${comment}${entry}\n`);
}

// Add or replace one server under `mcpServers` in an agent's JSON config, keeping the user's
// own servers and keys. `mode` applies only when the file is created; an existing file keeps
// its permissions.
export function upsertMcpServer(
  file: string,
  name: string,
  server: Record<string, unknown>,
  opts: { mode?: number } = {},
): void {
  const config = readJsonObject(file);
  config.mcpServers = { ...serversOf(config), [name]: server };
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(config, null, 2)}\n`, { mode: opts.mode });
}

// Editor and agent files that scene templates ship without a leading dot, which no default deploy
// rule covers: sdk-empty-scene-template has `cursor.config` and `cursorignore` at its root.
// Dot-files and dot-folders (`.cursor/`, `.claude/`, `.vscode/`, `.mcp.json`) and Markdown
// (`AGENTS.md`, `CLAUDE.md`) need no entry: sdk-commands always adds `.*` and `*.md`.
export const EDITOR_FILES_DCLIGNORE = ['cursor.config', 'cursorignore'];

const EDITOR_FILES_COMMENT = '# Editor and agent files, never part of the deployed scene';

// Make sure the scene's `.dclignore` keeps `EDITOR_FILES_DCLIGNORE` out of the deploy. Appends
// only the entries no line already names (an `!entry` line is the creator's choice and is kept),
// and never rewrites what the creator wrote. A scene without a `.dclignore` gets one that starts
// with the deploy's default rules, so the file reads as the whole list.
export function ensureEditorFilesNotDeployed(dir: string): void {
  const dclignore = path.join(dir, '.dclignore');
  let content: string | null = null;
  try {
    content = fs.readFileSync(dclignore, 'utf8');
  } catch {
    /* no .dclignore yet — created below */
  }
  const named = new Set(
    parseDclignore(content ?? '').map(line => line.replace(/^!/, '').replace(/^\//, '')),
  );
  const missing = EDITOR_FILES_DCLIGNORE.filter(entry => !named.has(entry));
  if (missing.length === 0) return;
  const head =
    content === null
      ? `${DEFAULT_DCLIGNORE.join('\n')}\n`
      : content === '' || content.endsWith('\n')
        ? content
        : `${content}\n`;
  fs.writeFileSync(dclignore, `${head}${EDITOR_FILES_COMMENT}\n${missing.join('\n')}\n`);
}

// npm `overrides` a new scene gets on top of its template's `package.json`. `@dcl/sdk` 7.33.0
// (what the templates' `latest` installs) pulls protobufjs 6.11 and 7.2 through `@dcl/hashing`
// and `@dcl/protocol`, which `npm audit` reports as critical. 7.6.6 is the release the SDK's
// own workspace pins; it hashes deployed files identically and the scene still builds. The
// range selector leaves a fixed protobufjs (a later SDK, or protobufjs 8) alone.
export const SCENE_DEPENDENCY_OVERRIDES: Record<string, string> = {
  'protobufjs@<7.6.3': '^7.6.6',
};

const overriddenPackage = (key: string) => key.replace(/^(@?[^@]+)@.*$/, '$1');

// Add `SCENE_DEPENDENCY_OVERRIDES` to the scene's `package.json`, leaving any override the
// template or creator already set for the same package. A missing or unreadable `package.json`
// is left as it is.
export function addDependencyOverrides(dir: string): void {
  const file = path.join(dir, 'package.json');
  if (!fs.existsSync(file)) return;
  const pkg = readJsonObject(file);
  if (Object.keys(pkg).length === 0) return;
  const overrides =
    pkg.overrides !== null && typeof pkg.overrides === 'object' && !Array.isArray(pkg.overrides)
      ? (pkg.overrides as Record<string, unknown>)
      : {};
  const present = new Set(Object.keys(overrides).map(overriddenPackage));
  const added = Object.entries(SCENE_DEPENDENCY_OVERRIDES).filter(
    ([key]) => !present.has(overriddenPackage(key)),
  );
  if (added.length === 0) return;
  pkg.overrides = { ...overrides, ...Object.fromEntries(added) };
  fs.writeFileSync(file, `${JSON.stringify(pkg, null, 2)}\n`);
}
