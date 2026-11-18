import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { killInspectorServer, start } from './inspector';

const state = vi.hoisted(() => ({ appPath: '' }));

vi.mock('electron', () => ({ app: { getAppPath: () => state.appPath } }));
vi.mock('electron-log', () => ({ default: { info: vi.fn(), error: vi.fn() } }));
vi.mock('./cache', () => ({ getPreview: vi.fn() }));

const ISOLATION_HEADERS = {
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-embedder-policy': 'credentialless',
  'cross-origin-resource-policy': 'cross-origin',
};

describe('start', () => {
  let folder: string;
  let port: number;
  // Copied in `beforeAll`: when a test runs, the spy no longer holds the call.
  let listenCalls: unknown[][];

  beforeAll(async () => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), 'inspector-server-'));
    // In a development run the inspector is at `<app path>/../../packages/inspector/public`.
    state.appPath = path.join(folder, 'packages', 'creator-hub');
    const publicFolder = path.join(folder, 'packages', 'inspector', 'public');
    fs.mkdirSync(path.join(publicFolder, 'bevy-agent', 'bevy-agent'), { recursive: true });
    fs.writeFileSync(path.join(publicFolder, 'index.html'), '<title>inspector</title>');
    fs.writeFileSync(
      path.join(publicFolder, 'bevy-agent', 'bevy-agent', 'about'),
      '{"baseUrl":"http://__ORIGIN__/bevy-agent/"}',
    );

    const listen = vi.spyOn(http.Server.prototype, 'listen');
    port = await start();
    listenCalls = [...listen.mock.calls];
  });

  afterAll(() => {
    killInspectorServer();
    vi.restoreAllMocks();
    fs.rmSync(folder, { recursive: true, force: true });
  });

  it('should listen on the loopback address only', () => {
    expect(listenCalls).toEqual([[port, '127.0.0.1', expect.any(Function)]]);
  });

  it('should serve the inspector with the isolation headers', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/`);

    expect(response.status).toBe(200);
    expect(await response.text()).toBe('<title>inspector</title>');
    expect(Object.fromEntries(response.headers)).toMatchObject(ISOLATION_HEADERS);
  });

  it('should put the origin of the server in the agent realm manifest', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/bevy-agent/bevy-agent/about?x=1`);

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ baseUrl: `http://localhost:${port}/bevy-agent/` });
    expect(Object.fromEntries(response.headers)).toMatchObject({
      ...ISOLATION_HEADERS,
      'content-type': 'application/json',
    });
  });

  it('should refuse a method other than GET and HEAD for the agent realm manifest', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/bevy-agent/bevy-agent/about`, {
      method: 'POST',
    });

    expect(response.status).toBe(405);
  });

  it('should answer 404 with the isolation headers for a missing file', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/missing.js`);

    expect(response.status).toBe(404);
    expect(Object.fromEntries(response.headers)).toMatchObject(ISOLATION_HEADERS);
  });
});
