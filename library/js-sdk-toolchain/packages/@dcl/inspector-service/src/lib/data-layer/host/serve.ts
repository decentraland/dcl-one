import { createRpcServer } from '@dcl/rpc';
import type { ILogger } from '@dcl/rpc/dist/server';
import { registerService } from '@dcl/rpc/dist/codegen';
import { WebSocketTransport } from '@dcl/rpc/dist/transports/WebSocket';
import { WebSocketServer } from 'ws';
import type { DataLayerContext, FileSystemInterface } from '../types';
import { DataServiceDefinition } from '../proto/gen/data-layer.gen';
import type { DataLayerHost } from '.';

export type ServeDataLayerOptions = {
  /** Interface to listen on; 127.0.0.1 by default. */
  host?: string;
  /** Port to listen on; 0 (an ephemeral one) by default. */
  port?: number;
  logger?: ILogger;
};

export type DataLayerServer = {
  port: number;
  close(): Promise<void>;
};

/**
 * Serves a data-layer host over WebSocket: every connection gets an RPC port carrying
 * `DataServiceDefinition` over `host.rpcMethods`, with the file system and the host's engine as
 * the call context. This is the server `@dcl/sdk-commands start --data-layer` runs, packaged so a
 * driver needs nothing but this bundle.
 */
export async function serveDataLayer(
  host: DataLayerHost,
  fs: FileSystemInterface,
  options: ServeDataLayerOptions = {},
): Promise<DataLayerServer> {
  const rpcServer = createRpcServer<DataLayerContext>({ logger: options.logger });
  rpcServer.setHandler(async serverPort => {
    registerService(serverPort, DataServiceDefinition, async () => host.rpcMethods);
  });
  const wss = new WebSocketServer({ host: options.host ?? '127.0.0.1', port: options.port ?? 0 });
  wss.on('connection', ws => {
    rpcServer.attachTransport(WebSocketTransport(ws as any), { fs, engine: host.engine });
    ws.on('error', () => ws.close());
  });
  await new Promise<void>((resolve, reject) => {
    wss.once('listening', () => resolve());
    wss.once('error', reject);
  });
  const address = wss.address();
  const port = typeof address === 'object' && address ? address.port : Number(options.port);
  return {
    port,
    close: () =>
      new Promise<void>((resolve, reject) => {
        for (const client of wss.clients) client.terminate();
        wss.close(err => (err ? reject(err) : resolve()));
      }),
  };
}
