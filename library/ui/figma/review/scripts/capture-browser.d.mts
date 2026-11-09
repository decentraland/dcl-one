type CaptureSession = {
  runtimeErrors: string[]
  consoleErrors: string[]
  send: (method: string, params?: Record<string, unknown>) => Promise<unknown>
  close: () => Promise<void>
}

export declare function findBrowser(candidates: (string | undefined)[]): Promise<string>
export declare function openSession(
  executable: string,
  options?: { signal?: AbortSignal }
): Promise<CaptureSession>
