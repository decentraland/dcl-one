import path from 'node:path'
import { createLogger, createServer, type Logger, type ViteDevServer } from 'vite'

/** A Vite dev server that serves one consumer test page at `/`, and what went wrong while it served it. */
export interface ConsumerPage {
  server: ViteDevServer
  /** Dependency-scan errors and dependency re-optimizations, each of which reloads an open page. */
  disruptions: string[]
}

const DISRUPTION =
  /Failed to run dependency scan|new dependencies optimized|optimized dependencies changed/

/**
 * Serves `head` and the module `entry` (a path under library/ui, such as `test/consumer/button-browser.tsx`) from
 * library/ui. The dependency scan reads only `entry`: by default Vite scans every HTML file under the root, the
 * gallery's HUD page among them, whose host aliases this server does not define. That scan fails, pre-bundling is
 * skipped, and the dependencies found while the page loads make Vite reload it mid-test. Each page also keeps its
 * own dependency cache, since the consumer pages run side by side.
 */
export async function serveConsumerPage(entry: string, head: string): Promise<ConsumerPage> {
  const disruptions: string[] = []
  const base = createLogger('error', { allowClearScreen: false })
  const record =
    (forward: (message: string, options?: object) => void) =>
    (message: string, options?: object) => {
      if (DISRUPTION.test(message)) disruptions.push(message.split('\n')[0]!)
      forward(message, options)
    }
  const customLogger: Logger = {
    ...base,
    info: record(() => {}),
    warn: record(base.warn),
    warnOnce: record(base.warnOnce),
    error: record(base.error)
  }
  const server = await createServer({
    configFile: false,
    root: path.resolve(import.meta.dirname, '..'),
    appType: 'custom',
    customLogger,
    // Its own dependency cache: the consumer pages run side by side, and pages with different dependencies
    // rewriting one shared cache would reload each other.
    cacheDir: path.resolve(
      import.meta.dirname,
      '../node_modules/.vite-consumer',
      path.basename(entry, '.tsx')
    ),
    optimizeDeps: { entries: [entry] },
    resolve: { dedupe: ['react', 'react-dom'] },
    server: { host: '127.0.0.1', port: 0 }
  })
  server.middlewares.use(async (request, response, next) => {
    if (new URL(request.url ?? '/', 'http://localhost').pathname !== '/') return next()
    response.setHeader('Content-Type', 'text/html')
    response.end(
      await server.transformIndexHtml(
        '/',
        `<!doctype html>${head}<div id="root"></div><script type="module" src="/${entry}"></script>`
      )
    )
  })
  await server.listen()
  return { server, disruptions }
}
