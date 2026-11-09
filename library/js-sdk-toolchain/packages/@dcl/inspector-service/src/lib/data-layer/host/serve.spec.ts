import { createRpcClient } from '@dcl/rpc';
import { loadService } from '@dcl/rpc/dist/codegen';
import { WebSocketTransport } from '@dcl/rpc/dist/transports/WebSocket';
import { WebSocket } from 'ws';
import { feededFileSystem } from '../client/feeded-local-fs';
import { createDataLayerHost } from '.';
import { DataServiceDefinition } from '../proto/gen/data-layer.gen';
import { serveDataLayer } from './serve';
import type { DataLayerRpcClient } from '../types';

describe('serveDataLayer', () => {
  it('serves the host over WebSocket to an rpc client', async () => {
    const { fs } = await feededFileSystem();
    const host = await createDataLayerHost(fs);
    const server = await serveDataLayer(host, fs);
    expect(server.port).toBeGreaterThan(0);
    const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
    await new Promise<void>((resolve, reject) => {
      ws.once('open', () => resolve());
      ws.once('error', reject);
    });
    const client = await createRpcClient(WebSocketTransport(ws as any));
    const port = await client.createPort('data-layer');
    const dataLayer = loadService(port, DataServiceDefinition) as DataLayerRpcClient;
    const preferences = await dataLayer.getInspectorPreferences({});
    expect(preferences).toMatchObject({ autosaveEnabled: true });
    ws.close();
    await server.close();
  });

  it('closes with its clients', async () => {
    const { fs } = await feededFileSystem();
    const host = await createDataLayerHost(fs);
    const server = await serveDataLayer(host, fs);
    const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
    await new Promise<void>(resolve => ws.once('open', () => resolve()));
    const closed = new Promise<void>(resolve => ws.once('close', () => resolve()));
    await server.close();
    await closed;
    await expect(
      new Promise((_, reject) => new WebSocket(`ws://127.0.0.1:${server.port}`).once('error', reject)),
    ).rejects.toBeTruthy();
  });
});
