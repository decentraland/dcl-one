export interface CoreHttpSnapshot {
  active: number
  highWater: number
  oldestActiveMs: number
  suspectedStarvation: boolean
}
interface CoreHttpClient {
  begin(id: number): void
  end(id: number): void
  nextId(): number
  dispose(): void
}
export interface CoreHttpHost extends CoreHttpClient {
  connect(port: MessagePort): (() => void) | undefined
  snapshot(): CoreHttpSnapshot
}
export function installCoreHttp(): CoreHttpHost | CoreHttpClient
export function coreFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
export function coreFetch<T>(input: RequestInfo | URL, init: RequestInit | undefined, read: (response: Response) => T | Promise<T>): Promise<T>
export function coreFetchJson<T = unknown>(input: RequestInfo | URL, init?: RequestInit, errorPrefix?: string): Promise<T>
export function observeCoreHttp<T>(transfer: () => Promise<T>): Promise<T>
export function installHudHttp(inShell: boolean): void
export function connectShellHttp(port: MessagePort): void
export function disposeCoreHttp(): void
