type CaptureSession = {
  runtimeErrors: string[]
  consoleErrors: string[]
  /** Navigations the top frame requested or started since the session opened. */
  readonly navigations: number
  /** True from the top frame's frameStartedLoading to its frameStoppedLoading. */
  readonly loading: boolean
  send: (method: string, params?: Record<string, unknown>) => Promise<unknown>
  /** Sends Page.navigate and resolves once the top frame has finished loading. */
  navigate: (url: string, timeout?: number) => Promise<unknown>
  /** Resolves once the top frame is not loading; rejects after `timeout` ms (30 s by default). */
  whenLoaded: (timeout?: number) => Promise<void>
  close: () => Promise<void>
}

/** A command was in flight in a document that an observed navigation of the top frame replaced. */
export declare class NavigationInterrupted extends Error {}
export declare function findBrowser(candidates: (string | undefined)[]): Promise<string>
export declare function openSession(
  executable: string,
  options?: { signal?: AbortSignal }
): Promise<CaptureSession>
