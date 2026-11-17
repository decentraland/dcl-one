type CheckBuildSummary = {
  entry: string
  components: number
  component: string
  variant: number
  audit: { width: number; height: number }
  requests: number
}

export declare function checkBuild(options: {
  dist: string
  prefix?: string
  browser?: string
  /** Starts the browser session; tests inject one. Defaults to capture-browser's openSession. */
  open?: typeof import('./capture-browser.mjs').openSession
  /** Builds the HTTP server; tests inject one. Defaults to node:http createServer. */
  createServer?: typeof import('node:http').createServer
}): Promise<CheckBuildSummary>
export declare function main(argv: string[]): Promise<number>
