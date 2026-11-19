import type { Child, StreamType } from './bin';
import { createLineBuffer } from './bevy-realm-build-events';

/**
 * The Multiplayer Server lines of the Bevy editor realm, per scene.
 *
 * A preview started with `--no-server` (cli.ts) shares the server the realm spawned, so
 * that server's logs land in the realm's output, not the preview's. bevy-realm.ts feeds
 * each realm it starts in here, and the scene debugger (inspector.ts) subscribes by scene
 * path. A subscription therefore outlives a realm restart, and works when the console
 * opens before the realm is up.
 */

// What `sdk-commands start` prints about its Multiplayer Server: the server's own output,
// which it tags `[Server]`, and its warnings about the server process.
const SERVER_LINE = /\[Server\]|Multiplayer Server/;

// The realm's own output buffer is mostly build lines, so the server lines a console opened
// later replays are kept here instead.
const MAX_BACKLOG_LINES = 500;

// Tells `sdk-commands start` not to spawn its own Multiplayer Server.
export const NO_SERVER_FLAG = '--no-server';

/** Whether a preview was started to leave the Multiplayer Server to the editor realm. */
export function usesRealmServer(preview: Child): boolean {
  return preview.args.includes(NO_SERVER_FLAG);
}

export const REALM_SERVER_STOPPED =
  "⚠️ WARNING! The editor's Multiplayer Server stopped. Restart the preview to run the scene with a Multiplayer Server.";

type Listener = (text: string) => void;
type Channel = { backlog: string[]; listeners: Set<Listener> };

const channels = new Map<string, Channel>();

function getChannel(path: string): Channel {
  let channel = channels.get(path);
  if (!channel) {
    channel = { backlog: [], listeners: new Set() };
    channels.set(path, channel);
  }
  return channel;
}

function toText(lines: string[]): string {
  return lines.map(line => `${line}\n`).join('');
}

function publish(path: string, lines: string[]): void {
  const channel = getChannel(path);
  channel.backlog.push(...lines);
  channel.backlog.splice(0, Math.max(0, channel.backlog.length - MAX_BACKLOG_LINES));
  const text = toText(lines);
  for (const listener of channel.listeners) listener(text);
}

/** Collects the Multiplayer Server lines of a realm child. A new realm starts a new backlog. */
export function captureRealmServerLogs(path: string, child: Child): void {
  getChannel(path).backlog = [];
  // One line buffer per stream: a stderr chunk arriving inside a stdout line must not cut it.
  const streams: StreamType[] = ['stdout', 'stderr'];
  for (const type of streams) {
    const lines = createLineBuffer();
    child.on(
      /(.*)/,
      data => {
        if (!data) return;
        const serverLines = lines.push(data).filter(line => SERVER_LINE.test(line));
        if (serverLines.length > 0) publish(path, serverLines);
      },
      { type, sanitize: false },
    );
  }
}

/** Tells the consoles that share the realm's server that it is gone. */
export function realmServerStopped(path: string): void {
  publish(path, [REALM_SERVER_STOPPED]);
}

/** Receives each new batch of server lines for the scene. Returns the unsubscribe. */
export function subscribeRealmServerLogs(path: string, listener: Listener): () => void {
  const { listeners } = getChannel(path);
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The server lines kept so far for the scene, complete lines only. */
export function getRealmServerBacklog(path: string): string {
  return toText(channels.get(path)?.backlog ?? []);
}
