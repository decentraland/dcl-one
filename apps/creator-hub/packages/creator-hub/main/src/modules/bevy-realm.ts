import log from 'electron-log/main';

import {
  BEVY_REALM_BUILD_EVENT,
  BEVY_REALM_SUPERVISION_EVENT,
  type BevyRealmBuildEvent,
  type BevyRealmSupervisionEvent,
} from '/shared/types/ipc';

import { MAIN_WINDOW_ID } from '../mainWindow';
import { run, type Child } from './bin';
import { getAvailablePort } from './port';
import { getProjectId, track } from './analytics';
import { getWindow } from './window';
import { createLineBuffer, parseSceneBuildEvents } from './bevy-realm-build-events';
import { clearAllRealmDown, clearRealmDown, markRealmDown } from './realm-status';
import {
  NO_SERVER_FLAG,
  REALM_SERVER_PREVIEW_HINT,
  REALM_SERVER_RESTARTING,
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
 * to the editor (BEVY_REALM_SUPERVISION_EVENT). A deliberate stop never restarts.
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
};

export const MAX_RESTARTS = 3;
export const RESTART_WINDOW_MS = 60_000;
const RESTART_BACKOFF_MS = [1_000, 3_000, 9_000];

// sdk-commands exits with this code when the bevy server cannot run on this machine; a
// restart would only fail the same way.
const SERVER_UNAVAILABLE_CODE = '78';

// What `sdk-commands start` prints when its Multiplayer Server child stops. The realm keeps
// running without it, so its own exit does not tell. A SIGTERM/SIGINT stop prints nothing:
// that is a deliberate stop. The server's own lines carry the `[Server]` tag and are skipped.
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

// A realm whose Multiplayer Server stopped still serves the editor, but it is no longer
// what the project should run: start() and ensure() replace it.
function isRealmRunning(realm?: Realm): realm is Realm {
  return !!(realm?.child.alive() && realm.url && !realm.serverDown);
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
  if (isRealmRunning(realm)) return realm.servesMultiplayer;
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
  await stopRealm(path);

  const port = await getAvailablePort();
  const url = `http://localhost:${port}`;
  const wsUrl = `ws://localhost:${port}/data-layer`;
  const spawnsServer = await spawnsMultiplayerServer(path);
  // A preview that already runs its own server keeps it; a second one would lose the room.
  const leavesServer = spawnsServer && previewRunsServer(path);

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
    env: await getEnv(path),
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
    throw err;
  }

  log.info(`[BevyRealm] Serving ${path} at ${url}`);
  clearRealmDown(path);
  watchExit(path, child);
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

function watchExit(path: string, child: Child): void {
  const servingSince = Date.now();
  const onExit = (message: string) => {
    const realm = realms.get(path);
    if (shuttingDown || realm?.child !== child) return;
    realms.delete(path);
    markRealmDown(path);
    // A stopped server was handled already, including the realm exit that may follow it
    // (sdk-commands exits after an unavailable server).
    if (realm.serverDown || restartPending(path)) return;
    log.error(
      `[BevyRealm] ${path} exited unexpectedly after ${Date.now() - servingSince} ms: ${message}`,
    );
    const restartInMs = scheduleRestart(path);
    if (realm.servesMultiplayer) {
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
  const lines = createLineBuffer();
  realm.child.on(/(.*)/, data => {
    if (!data) return;
    for (const line of lines.push(data)) {
      if (line.includes('[Server]')) continue;
      const match = SERVER_STOPPED.exec(line);
      if (match) onServerStopped(path, realm, line.trim(), match[1]);
    }
  });
}

function onServerStopped(path: string, realm: Realm, message: string, exitCode?: string): void {
  if (shuttingDown || realms.get(path) !== realm || realm.serverDown) return;
  realm.serverDown = true;
  log.error(`[BevyRealm] The Multiplayer Server of ${path} stopped: ${message}`);
  if (restartPending(path)) return;
  const restartInMs = scheduleRestart(path, exitCode === SERVER_UNAVAILABLE_CODE);
  realmServerStopped(
    path,
    restartInMs === null ? REALM_SERVER_PREVIEW_HINT : REALM_SERVER_RESTARTING,
  );
  notify({ kind: 'stopped', path, cause: 'server', message, restartInMs });
}

/** Plans the next restart of the realm. Returns its delay, or null when it gave up. */
function scheduleRestart(path: string, unavailable = false): number | null {
  let supervisor = supervisors.get(path);
  if (!supervisor) {
    supervisor = { restarts: [], restarting: false, gaveUp: false };
    supervisors.set(path, supervisor);
  }
  const now = Date.now();
  const plan = unavailable ? null : planRestart(supervisor.restarts, now);
  if (!plan) {
    supervisor.gaveUp = true;
    log.error(`[BevyRealm] Not restarting ${path}: it stopped too often, or cannot run here`);
    return null;
  }
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
    notify({ kind: 'stopped', path, cause: 'realm', message, restartInMs: scheduleRestart(path) });
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

async function stopRealm(path: string): Promise<void> {
  starting.delete(path);
  const realm = realms.get(path);
  const promise = realm?.child.kill().catch(() => {});
  realms.delete(path);
  if (realm?.servesMultiplayer && !realm.serverDown && !shuttingDown) {
    realmServerStopped(path, REALM_SERVER_PREVIEW_HINT);
  }
  await promise;
}

/**
 * Gets a serving realm for the project, as when the editor reloads: reuses a healthy one,
 * replaces one that stopped or lost its Multiplayer Server, and starts the restart count over.
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
    const message = error instanceof Error ? error.message : String(error);
    notify({ kind: 'stopped', path, cause: 'realm', message, restartInMs: null });
    throw error;
  }
}

export async function kill(path: string): Promise<void> {
  dropSupervisor(path);
  await stopRealm(path);
  clearRealmDown(path);
}

export async function killAllRealms(): Promise<void> {
  shuttingDown = true;
  try {
    for (const path of [...supervisors.keys()]) dropSupervisor(path);
    for (const path of realms.keys()) {
      await kill(path);
    }
    realms.clear();
    clearAllRealmDown();
  } finally {
    shuttingDown = false;
  }
}
