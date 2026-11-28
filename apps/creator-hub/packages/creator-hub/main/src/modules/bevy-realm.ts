import { basename } from 'node:path';

import { watch, type FSWatcher } from 'chokidar';
import log from 'electron-log/main';

import {
  BEVY_REALM_BUILD_EVENT,
  BEVY_REALM_SUPERVISION_EVENT,
  type BevyRealmBuildEvent,
  type BevyRealmSupervisionEvent,
} from '/shared/types/ipc';

import { MAIN_WINDOW_ID } from '../mainWindow';
import { run, type Child, type StreamType } from './bin';
import { getAvailablePort } from './port';
import { getProjectId, track } from './analytics';
import { getWindow } from './window';
import { createLineBuffer, parseSceneBuildEvents } from './bevy-realm-build-events';
import { clearAllRealmDown, clearRealmDown, markRealmDown } from './realm-status';
import {
  NO_SERVER_FLAG,
  REALM_SERVER_PREVIEW_HINT,
  REALM_SERVER_RESTARTING,
  REALM_SERVER_UNAVAILABLE,
  captureRealmServerLogs,
  previewRunsServer,
  realmServerStopped,
  spawnsMultiplayerServer,
} from './realm-server-logs';

/**
 * The Bevy editor renderer loads the scene from an HTTP realm, and the inspector
 * shares that realm's data-layer WebSocket so entity ids align with the engine
 * (forward edits land on the right entities). This module owns a headless
 * `sdk-commands start` per project that serves exactly that: the scene content +
 * a `/data-layer` WS, with nothing auto-launched.
 *
 *   sdk-commands start --port <p> --no-browser --no-client --data-layer
 *
 * It's the same server the dev testing setup runs by hand — see the
 * `bevy-renderer-testing-setup` runbook. `--no-client` suppresses every
 * auto-launch (no deep-link, no browser); `--data-layer` is load-bearing:
 * without it `/data-layer` 404s and the inspector tree shows default entities
 * instead of the scene.
 *
 * Lifecycle: one server per project path, started when the Bevy renderer becomes
 * active for that project and killed when the project closes (or on app quit via
 * {@link killAllRealms}). Distinct from the preview server in cli.ts — that one
 * launches a client for the user; this one only feeds the embedded editor engine.
 *
 * On an SDK with the authoritative server, `sdk-commands start` also spawns the scene's
 * Multiplayer Server as its own child, and the preview reuses it (cli.ts). When the realm
 * or only that server stops unexpectedly, this module restarts the realm with a backoff,
 * at most {@link MAX_RESTARTS} times in {@link RESTART_WINDOW_MS}, and reports each step
 * to the editor (BEVY_REALM_SUPERVISION_EVENT). A deliberate stop never restarts. A server
 * that cannot run on this machine is not restarted: the realm goes on without it.
 *
 * A realm also restarts when the scene's `package.json` or `package-lock.json` changes on disk
 * (an install from a terminal, an agent, the Hub's own SDK update). `sdk-commands start` works
 * out where `@dcl/sdk` and `@dcl/ecs` live once, when it starts, and its file watcher skips
 * `node_modules`, so after an install moved or removed a package every rebuild failed with
 * "Could not resolve" until the scene was reopened. See {@link watchDependencies}.
 */

type Realm = {
  child: Child;
  /** e.g. `http://localhost:8004` — fed to the inspector's `bevyRealm` config. */
  url: string;
  /** e.g. `ws://localhost:8004/data-layer` — the inspector's `dataLayerRpcWsUrl`. */
  wsUrl: string;
  port: number;
  /** It spawned the scene's Multiplayer Server (it left it to a preview otherwise). */
  servesMultiplayer: boolean;
  /** Its Multiplayer Server stopped while the realm kept serving. */
  serverDown: boolean;
  /** It printed its ready line: the editor can load from it. False while it boots. */
  ready: boolean;
  /** Watches the scene's dependency files once it serves; see {@link watchDependencies}. */
  dependencies?: { watcher: FSWatcher; timer?: ReturnType<typeof setTimeout> };
};

// The files an install writes when it is done with `node_modules`. npm writes both at the end
// of an install, and an edit by hand that changes the dependencies goes through `package.json`.
export const DEPENDENCY_FILES = ['package.json', 'package-lock.json'];

// Quiet time after the last change before the realm restarts, so the two writes of one install
// (and a burst of installs, as when an agent adds packages one by one) cost one restart.
export const DEPENDENCY_SETTLE_MS = 2_000;

export const MAX_RESTARTS = 3;
export const RESTART_WINDOW_MS = 60_000;
const RESTART_BACKOFF_MS = [1_000, 3_000, 9_000];

// sdk-commands exits with this code when the bevy server cannot run on this machine; a
// restart would only fail the same way.
const SERVER_UNAVAILABLE_CODE = '78';

// What `sdk-commands start` prints when its Multiplayer Server child stops. The realm keeps
// running without it, so its own exit does not tell. A SIGTERM/SIGINT stop prints nothing:
// that is a deliberate stop, and so is exit code 0. The server's own lines carry the
// `[Server]` tag and are skipped. The wording is `printWarning` in the `close` and `error`
// handlers of decentraland/js-sdk-toolchain
// `packages/@dcl/sdk-commands/src/commands/start/multiplayer-server.ts`, and the test
// fixtures in bevy-realm.test.ts copy it. If it changes, a server-only stop goes
// unnoticed (the realm keeps serving without a server); a realm crash is still restarted.
const SERVER_STOPPED =
  /Multiplayer Server (?:exited with code (\d+)|terminated by signal|process error)/;

/**
 * When to restart after an unexpected stop, given when the earlier restarts were planned.
 * Returns null once {@link MAX_RESTARTS} fall inside {@link RESTART_WINDOW_MS}.
 */
export function planRestart(earlier: number[], now: number): { delayMs: number } | null {
  const recent = earlier.filter(at => now - at < RESTART_WINDOW_MS).length;
  if (recent >= MAX_RESTARTS) return null;
  return { delayMs: RESTART_BACKOFF_MS[Math.min(recent, RESTART_BACKOFF_MS.length - 1)] };
}

// Restart bookkeeping per path. Created on the first unexpected stop, dropped by a
// deliberate stop (kill, app quit) and replaced by ensure(), which starts the count over.
type Supervisor = {
  restarts: number[];
  timer?: ReturnType<typeof setTimeout>;
  restarting: boolean;
  gaveUp: boolean;
};

const supervisors: Map<string, Supervisor> = new Map();

const realms: Map<string, Realm> = new Map();
// In-flight start() per path. Serializes concurrent starts (a rapid renderer
// toggle) so a second caller reuses the first's pending server instead of
// spawning a duplicate, and lets kill() cancel a start that hasn't resolved yet.
const starting: Map<string, Promise<{ url: string; wsUrl: string }>> = new Map();
let shuttingDown = false;

// Deliberate stops per path. A start reads the count before it prepares and spawns nothing
// when it changed meanwhile: the realm would outlive the editor that asked for it.
const deliberateStops: Map<string, number> = new Map();

// Scenes whose Multiplayer Server cannot run on this machine (SERVER_UNAVAILABLE_CODE). Their
// realm starts without it until the scene is closed.
const serverUnavailable: Set<string> = new Set();

class StartCancelled extends Error {
  constructor() {
    super('The scene server was stopped before it started.');
  }
}

// A realm whose Multiplayer Server stopped still serves the editor, but it is no longer
// what the project should run: start() and ensure() replace it. A realm that is still
// booting is not running yet: start() and ensure() wait for its start instead.
function isRealmRunning(realm?: Realm): realm is Realm {
  return !!(realm?.child.alive() && realm.url && realm.ready && !realm.serverDown);
}

export function getRealm(path: string): Realm | undefined {
  const realm = realms.get(path);
  return isRealmRunning(realm) ? realm : undefined;
}

/**
 * Whether the editor realm runs the scene's Multiplayer Server, or will once a pending
 * restart lands. A preview started now leaves the server to it (cli.ts).
 */
export function realmProvidesServer(path: string): boolean {
  const realm = realms.get(path);
  // A booting realm counts: its server comes up with it.
  if (realm?.child.alive() && !realm.serverDown) return realm.servesMultiplayer;
  const supervisor = supervisors.get(path);
  return !!supervisor && !supervisor.gaveUp && (!!supervisor.timer || supervisor.restarting);
}

async function getEnv(path: string) {
  const projectId = await getProjectId(path);
  return {
    ANALYTICS_PROJECT_ID: projectId,
    ANALYTICS_APP_ID: 'creator-hub',
  };
}

/**
 * Start (or reuse) the Bevy realm server for a project. Resolves once the server
 * is serving, returning the realm + data-layer URLs the inspector needs.
 */
export function start(path: string): Promise<{ url: string; wsUrl: string }> {
  const existing = realms.get(path);
  if (isRealmRunning(existing)) {
    return Promise.resolve({ url: existing.url, wsUrl: existing.wsUrl });
  }

  // Coalesce concurrent starts for the same path so we never spawn two servers
  // for one project (the second toggle reuses the first's pending start).
  const inFlight = starting.get(path);
  if (inFlight) return inFlight;

  const promise = startInternal(path).finally(() => {
    // Only clear if still ours — kill() or a later start may have replaced it.
    if (starting.get(path) === promise) starting.delete(path);
  });
  starting.set(path, promise);
  return promise;
}

async function startInternal(path: string): Promise<{ url: string; wsUrl: string }> {
  const stopsBefore = deliberateStops.get(path) ?? 0;
  await stopRealm(path);

  const port = await getAvailablePort();
  const url = `http://localhost:${port}`;
  const wsUrl = `ws://localhost:${port}/data-layer`;
  const spawnsServer = await spawnsMultiplayerServer(path);
  const env = await getEnv(path);
  if ((deliberateStops.get(path) ?? 0) !== stopsBefore) throw new StartCancelled();
  // A preview that already runs its own server keeps it; a second one would lose the room.
  const leavesServer = spawnsServer && (previewRunsServer(path) || serverUnavailable.has(path));

  const child = run('@dcl/sdk-commands', 'sdk-commands', {
    args: [
      'start',
      '--port',
      `${port}`,
      '--no-browser',
      '--no-client',
      '--data-layer',
      ...(leavesServer ? [NO_SERVER_FLAG] : []),
    ],
    cwd: path,
    workspace: path,
    env,
  });

  relayBuildEvents(path, child);
  captureRealmServerLogs(path, child);

  // Track the child IMMEDIATELY (before waiting for it to serve) so a kill(path)
  // during startup — the user switches renderer or closes the editor while the
  // server is still coming up — can find and terminate it. Storing only after
  // `waitFor` resolves leaves the pending process orphaned in that window.
  const realm: Realm = {
    child,
    url,
    wsUrl,
    port,
    servesMultiplayer: spawnsServer && !leavesServer,
    serverDown: false,
    ready: false,
  };
  realms.set(path, realm);

  // `--no-client` means no deep-link / browser open, so we wait for the
  // server-ready line sdk-commands prints once it's serving.
  const serverReady = /Preview server is now running/i;
  // waitFor only watches output, so a process that dies without printing a matching
  // line (a crash on boot, a kill mid-boot) would leave it pending forever.
  const exitedBeforeReady = child.wait().then(() => {
    throw new Error('The scene server exited before it was ready.');
  });
  exitedBeforeReady.catch(() => {});
  try {
    await Promise.race([child.waitFor(serverReady, /CliError|error:/i), exitedBeforeReady]);
  } catch (err) {
    // Startup failed or the child was killed mid-boot — make sure it's gone and
    // the entry doesn't linger as a half-alive realm. Guard the delete so a
    // concurrent kill()/start() that already replaced our entry isn't clobbered.
    await child.kill().catch(() => {});
    if (realms.get(path)?.child === child) realms.delete(path);
    // A deliberate stop while it booted killed it: not a crash.
    if ((deliberateStops.get(path) ?? 0) !== stopsBefore) throw new StartCancelled();
    throw err;
  }
  realm.ready = true;

  log.info(`[BevyRealm] Serving ${path} at ${url}`);
  clearRealmDown(path);
  watchExit(path, child);
  watchDependencies(path, realm);
  if (realm.servesMultiplayer) watchServer(path, realm);

  // Usage analytics (fire-and-forget): a realm only starts when the Bevy renderer is the
  // active renderer for this project, so one event per successful start is "Bevy renderer
  // used". Anonymous project id only — no scene content. start() coalesces concurrent
  // starts and reuses a running realm, so this fires once per activation, not per toggle.
  void getProjectId(path).then(project_id => track('Use Bevy Renderer', { project_id }));

  return { url, wsUrl };
}

/**
 * Forward the realm bundler's per-file rebuild lines to the renderer, which hands them
 * to the inspector's Bevy renderer. That is how the editor tells a code edit it must
 * hot-reload from a rebuild its own autosave caused (see BEVY_REALM_BUILD_EVENT).
 * The matcher dies with the child, so nothing to detach on kill.
 */
function relayBuildEvents(path: string, child: Child): void {
  // Match every chunk and split into lines ourselves: a pipe chunk can end mid-line,
  // and a matcher tested per chunk would miss a build line cut in two.
  const lines = createLineBuffer();
  child.on(/(.*)/, data => {
    if (!data) return;
    const events = lines.push(data).flatMap(parseSceneBuildEvents);
    if (events.length === 0) return;
    const window = getWindow(MAIN_WINDOW_ID);
    if (!window || window.isDestroyed()) return;
    for (const event of events) {
      const payload: BevyRealmBuildEvent = { path, ...event };
      window.webContents.send(BEVY_REALM_BUILD_EVENT, payload);
    }
  });
}

/**
 * Restarts the realm when the scene's dependencies change on disk, once they settle. The new
 * realm reads `node_modules` as it is now; the editor reloads on it like after any restart.
 */
function watchDependencies(path: string, realm: Realm): void {
  const watcher = watch(path, {
    depth: 0,
    ignoreInitial: true,
    // Only the project folder itself and its dependency files; nothing below it is walked.
    ignored: file => file !== path && !DEPENDENCY_FILES.includes(basename(file)),
  });
  const dependencies: NonNullable<Realm['dependencies']> = { watcher };
  realm.dependencies = dependencies;
  const onChange = (file: string) => {
    if (!DEPENDENCY_FILES.includes(basename(file))) return;
    if (dependencies.timer) clearTimeout(dependencies.timer);
    dependencies.timer = setTimeout(() => {
      dependencies.timer = undefined;
      void restartForDependencies(path, realm);
    }, DEPENDENCY_SETTLE_MS);
  };
  watcher
    .on('add', onChange)
    .on('change', onChange)
    .on('unlink', onChange)
    .on('error', err => log.warn(`[BevyRealm] Watching the dependencies of ${path}:`, err));
}

function unwatchDependencies(realm: Realm | undefined): void {
  const dependencies = realm?.dependencies;
  if (!dependencies) return;
  realm.dependencies = undefined;
  if (dependencies.timer) clearTimeout(dependencies.timer);
  void dependencies.watcher.close().catch(() => {});
}

async function restartForDependencies(path: string, realm: Realm): Promise<void> {
  // A realm that was replaced, stopped or is already being restarted starts from the files as
  // they are now anyway.
  if (shuttingDown || realms.get(path) !== realm || !isRealmRunning(realm)) return;
  if (restartPending(path) || starting.has(path)) return;
  log.info(`[BevyRealm] The dependencies of ${path} changed: restarting its server`);
  notify({ kind: 'restarting', path });
  try {
    await stopRealm(path, REALM_SERVER_RESTARTING);
    const started = await start(path);
    notify({ kind: 'restarted', path, ...started });
  } catch (error) {
    // The editor that asked has left the scene; there is no one to tell.
    if (error instanceof StartCancelled || shuttingDown) return;
    const message = error instanceof Error ? error.message : String(error);
    log.error(`[BevyRealm] Restarting ${path} after a dependency change failed: ${message}`);
    markRealmDown(path);
    notify({
      kind: 'stopped',
      path,
      cause: 'restart',
      message,
      restartInMs: scheduleRestart(path),
    });
  }
}

function watchExit(path: string, child: Child): void {
  const servingSince = Date.now();
  const onExit = (message: string) => {
    const realm = realms.get(path);
    if (shuttingDown || realm?.child !== child) return;
    realms.delete(path);
    unwatchDependencies(realm);
    markRealmDown(path);
    // The pending restart replaces this realm; its stop was reported with the server's.
    if (restartPending(path)) return;
    log.error(
      `[BevyRealm] ${path} exited unexpectedly after ${Date.now() - servingSince} ms: ${message}`,
    );
    const restartInMs = scheduleRestart(path);
    if (realm.servesMultiplayer && !realm.serverDown) {
      realmServerStopped(
        path,
        restartInMs === null ? REALM_SERVER_PREVIEW_HINT : REALM_SERVER_RESTARTING,
      );
    }
    notify({ kind: 'stopped', path, cause: 'realm', message, restartInMs });
  };
  void child.wait().then(
    () => onExit('The scene server exited.'),
    err => onExit(err instanceof Error ? err.message : String(err)),
  );
}

/** Restarts the realm when its Multiplayer Server child stops while the realm keeps serving. */
function watchServer(path: string, realm: Realm): void {
  // One line buffer per stream: a stderr chunk arriving inside a stdout line must not cut it.
  const streams: StreamType[] = ['stdout', 'stderr'];
  for (const type of streams) {
    const lines = createLineBuffer();
    realm.child.on(
      /(.*)/,
      data => {
        if (!data) return;
        for (const line of lines.push(data)) {
          if (line.includes('[Server]')) continue;
          const match = SERVER_STOPPED.exec(line);
          if (match) onServerStopped(path, realm, line.trim(), match[1]);
        }
      },
      { type },
    );
  }
}

function onServerStopped(path: string, realm: Realm, message: string, exitCode?: string): void {
  if (shuttingDown || realms.get(path) !== realm || realm.serverDown) return;
  if (!realm.servesMultiplayer) return;
  log.error(`[BevyRealm] The Multiplayer Server of ${path} stopped: ${message}`);
  if (exitCode === SERVER_UNAVAILABLE_CODE) {
    // A restart would only fail the same way. The realm goes on without the server, and a
    // realm that replaces it starts without one.
    serverUnavailable.add(path);
    realm.servesMultiplayer = false;
    realmServerStopped(path, REALM_SERVER_UNAVAILABLE);
    notify({
      kind: 'stopped',
      path,
      cause: 'server',
      message,
      restartInMs: null,
      unavailable: true,
    });
    return;
  }
  realm.serverDown = true;
  if (restartPending(path)) return;
  const restartInMs = scheduleRestart(path);
  realmServerStopped(
    path,
    restartInMs === null ? REALM_SERVER_PREVIEW_HINT : REALM_SERVER_RESTARTING,
  );
  notify({ kind: 'stopped', path, cause: 'server', message, restartInMs });
}

/** Plans the next restart of the realm. Returns its delay, or null when it gave up. */
function scheduleRestart(path: string): number | null {
  let supervisor = supervisors.get(path);
  if (!supervisor) {
    supervisor = { restarts: [], restarting: false, gaveUp: false };
    supervisors.set(path, supervisor);
  }
  const now = Date.now();
  const plan = planRestart(supervisor.restarts, now);
  if (!plan) {
    supervisor.gaveUp = true;
    log.error(`[BevyRealm] Not restarting ${path}: it stopped too often`);
    return null;
  }
  supervisor.gaveUp = false;
  supervisor.restarts.push(now);
  const owner = supervisor;
  owner.timer = setTimeout(() => void autoRestart(path, owner), plan.delayMs);
  return plan.delayMs;
}

function restartPending(path: string): boolean {
  const supervisor = supervisors.get(path);
  return !!supervisor && (!!supervisor.timer || supervisor.restarting);
}

async function autoRestart(path: string, supervisor: Supervisor): Promise<void> {
  supervisor.timer = undefined;
  if (shuttingDown || supervisors.get(path) !== supervisor) return;
  supervisor.restarting = true;
  notify({ kind: 'restarting', path });
  let realm: { url: string; wsUrl: string };
  try {
    realm = await start(path);
  } catch (error) {
    supervisor.restarting = false;
    // A deliberate stop during the restart lands here too; it dropped the supervisor.
    if (shuttingDown || supervisors.get(path) !== supervisor) return;
    const message = error instanceof Error ? error.message : String(error);
    log.error(`[BevyRealm] Restarting ${path} failed: ${message}`);
    notify({
      kind: 'stopped',
      path,
      cause: 'restart',
      message,
      restartInMs: scheduleRestart(path),
    });
    return;
  }
  supervisor.restarting = false;
  if (supervisors.get(path) !== supervisor) return;
  log.info(`[BevyRealm] Restarted ${path}`);
  notify({ kind: 'restarted', path, ...realm });
}

function dropSupervisor(path: string): void {
  const supervisor = supervisors.get(path);
  if (supervisor?.timer) clearTimeout(supervisor.timer);
  supervisors.delete(path);
}

function notify(event: BevyRealmSupervisionEvent): void {
  const window = getWindow(MAIN_WINDOW_ID);
  if (!window || window.isDestroyed()) return;
  window.webContents.send(BEVY_REALM_SUPERVISION_EVENT, event);
}

async function stopRealm(path: string, hint = REALM_SERVER_PREVIEW_HINT): Promise<void> {
  starting.delete(path);
  const realm = realms.get(path);
  const promise = realm?.child.kill().catch(() => {});
  realms.delete(path);
  unwatchDependencies(realm);
  if (realm?.servesMultiplayer && !realm.serverDown && !shuttingDown) {
    realmServerStopped(path, hint);
  }
  await promise;
}

/**
 * Gets a serving realm for the project, as when the editor reloads: reuses a healthy one,
 * replaces one that stopped or lost its Multiplayer Server, and starts the restart count over.
 * A realm that is still booting (an automatic restart, say) is awaited through start(), and
 * this reports it as restarted: the automatic restart it may belong to no longer will.
 */
export async function ensure(path: string): Promise<{ url: string; wsUrl: string }> {
  dropSupervisor(path);
  const realm = getRealm(path);
  if (realm) return { url: realm.url, wsUrl: realm.wsUrl };
  notify({ kind: 'restarting', path });
  try {
    const started = await start(path);
    notify({ kind: 'restarted', path, ...started });
    return started;
  } catch (error) {
    // The editor that asked has left the scene; there is no one to tell.
    if (error instanceof StartCancelled) throw error;
    const message = error instanceof Error ? error.message : String(error);
    notify({ kind: 'stopped', path, cause: 'realm', message, restartInMs: null });
    throw error;
  }
}

export async function kill(path: string): Promise<void> {
  dropSupervisor(path);
  deliberateStops.set(path, (deliberateStops.get(path) ?? 0) + 1);
  serverUnavailable.delete(path);
  await stopRealm(path);
  clearRealmDown(path);
}

export async function killAllRealms(): Promise<void> {
  shuttingDown = true;
  try {
    for (const path of [...supervisors.keys()]) dropSupervisor(path);
    for (const path of new Set([...starting.keys(), ...realms.keys()])) {
      await kill(path);
    }
    realms.clear();
    clearAllRealmDown();
  } finally {
    shuttingDown = false;
  }
}
