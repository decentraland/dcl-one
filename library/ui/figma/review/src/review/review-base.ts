// The base the app was built for: "/" locally, an absolute prefix such as /@scope/pkg/1.2.3/, or a relative base
// ("./"). Vite fills BASE_URL in at build time. It is read here, and only here, so the one place that depends on
// the bundler is easy to replace in a test; where it is absent (a Node test), the base is the root.
export const runtimeBase = (): string =>
  (import.meta as { env?: { BASE_URL?: string } }).env?.BASE_URL ?? '/'
