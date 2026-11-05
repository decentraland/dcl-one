import path from 'node:path'
import { copyFile, writeFile } from 'node:fs/promises'
import { defineConfig } from 'vite'
import { reviewSignature } from './scripts/review-signature.mjs'

export default defineConfig({
  root: import.meta.dirname,
  plugins: [
    {
      name: 'review-source-signature',
      apply: 'build',
      async closeBundle() {
        await copyFile(
          path.resolve(import.meta.dirname, 'review-dist/dcl-ui-review/index.html'),
          path.resolve(import.meta.dirname, 'review-dist/index.html')
        )
        await writeFile(
          path.resolve(import.meta.dirname, 'review-dist/dcl-ui-review/source-signature.json'),
          JSON.stringify({ signature: await reviewSignature(import.meta.dirname) })
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
