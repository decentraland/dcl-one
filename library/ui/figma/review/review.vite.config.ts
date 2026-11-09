import path from 'node:path'
import { readFile, writeFile } from 'node:fs/promises'
import { defineConfig } from 'vite'

let outDir = path.resolve(import.meta.dirname, 'review-dist')
let relativeBase = false

export default defineConfig({
  root: import.meta.dirname,
  plugins: [
    {
      name: 'review-root-entry',
      apply: 'build',
      configResolved(config) {
        outDir = path.resolve(config.root, config.build.outDir)
        relativeBase = config.base === '' || config.base.startsWith('.')
      },
      async closeBundle() {
        // The root entry sits one level above the generated page, so relative asset URLs move with it.
        const page = await readFile(path.resolve(outDir, 'dcl-ui-review/index.html'), 'utf8')
        await writeFile(
          path.resolve(outDir, 'index.html'),
          relativeBase ? page.replaceAll('"../dcl-ui-review/', '"./dcl-ui-review/') : page
        )
      }
    },
    {
      name: 'review-home',
      configureServer(server) {
        server.middlewares.use((request, response, next) => {
          if (request.url === '/' || request.url?.startsWith('/?')) {
            response.writeHead(302, { Location: `/dcl-ui-review/${request.url.slice(1)}` })
            response.end()
          } else next()
        })
      }
    }
  ],
  server: {
    host: '127.0.0.1',
    fs: { strict: true, allow: [path.resolve(import.meta.dirname, '../..')] }
  },
  resolve: {
    dedupe: ['react', 'react-dom']
  },
  build: {
    outDir: 'review-dist',
    emptyOutDir: true,
    assetsDir: 'dcl-ui-review/assets',
    rolldownOptions: { input: path.resolve(import.meta.dirname, 'dcl-ui-review/index.html') }
  }
})
