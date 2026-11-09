// The client's signing and transport contracts. Who signs, and how a request travels, are the host's: the
// browser's signers and transports are in api/host.ts.

export type ShopSigner = {
  /** Lowercase address the requests are signed for. */
  readonly address: string;
  /** Auth-chain headers for `method` + `path` (the exact pathname requested, no query). */
  sign(method: string, path: string, metadata: Record<string, unknown>): Promise<Record<string, string>>;
};

export type TransportRequest = {
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: string;
  signal?: AbortSignal;
  signed: boolean;
  signPath: string;
};

export type ShopTransport = (req: TransportRequest) => Promise<{ status: number; body: string }>;
