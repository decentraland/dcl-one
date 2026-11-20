import { readFileSync } from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const here = import.meta.dirname
// The Explorer page's core, which the HUD imports.
const explorerCore = path.resolve(here, '../../explorer-core')
/**
 * The names the Explorer HUD (apps/hud) compiles under, as its hosts give them (react-web's vite.shared.ts, Sites'
 * vite.config.ts), with the library's own HUD test host as `@hud-host`: a web host on decentraland.org that shows
 * pictures as given and has no Shop of its own. The HUD canvas and the `hud` Vitest project both use them, and
 * tsconfig.hud.json maps the same paths.
 */
export const hudAliases = {
  '@hud-host': path.resolve(here, '../apps/hud/test/host.ts'),
  '@hud': path.resolve(here, '../apps/hud'),
  '@explorer-core': explorerCore
}
// Only the names of the generated Review's components reach the bundle; the catalog itself stays in figma/.
const generatedComponents = () =>
  (
    JSON.parse(
      readFileSync(path.resolve(here, '../figma/review/src/review/catalog.json'), 'utf8')
    ) as { components: { name: string; variants: unknown[] }[] }
  ).components.map(({ name, variants }) => ({ name, variants: variants.length }))

// `--mode hosted` builds the gallery alone into gallery/dist, for a hosted preview: the Figma reference stays out,
// since it ships only in the Review artifact the privileged key scan checks.
export default defineConfig(({ command, mode }) => {
  const hosted = mode === 'hosted'
  return {
    root: here,
    // Relative URLs, so the build works under any prefix it is served from.
    base: './',
    // Its own dependency cache: `pnpm review` runs from the same package, and a shared node_modules/.vite would be
    // re-optimized for whichever server started last, breaking the other's prebundled dependencies.
    cacheDir: path.resolve(here, '../node_modules/.vite-gallery'),
    plugins: [react()],
    define: {
      __GENERATED_COMPONENTS__: JSON.stringify(hosted ? [] : generatedComponents()),
      // The build sits beside the generated Review in one output; `pnpm review` serves it in development.
      __GENERATED_REVIEW_URL__: JSON.stringify(
        hosted
          ? null
          : command === 'build'
            ? '../dcl-ui-review/'
            : (process.env.GALLERY_GENERATED_URL ?? 'http://127.0.0.1:6019/dcl-ui-review/')
      )
    },
    server: {
      host: '127.0.0.1',
      port: 6020,
      fs: { strict: true, allow: [path.resolve(here, '..'), explorerCore] }
    },
    resolve: { dedupe: ['react', 'react-dom'], alias: hudAliases },
    build: {
      // The hosted preview alone, or beside the generated Review's build (figma/review/review-dist), which the CI
      // artifact uploads whole.
      outDir: hosted
        ? path.resolve(here, 'dist')
        : path.resolve(here, '../figma/review/review-dist/gallery'),
      emptyOutDir: true,
      // The icons page bundles every foundation icon; a review tool loads it once.
      chunkSizeWarningLimit: 1500,
      rolldownOptions: {
        input: {
          index: path.resolve(here, 'index.html'),
          frame: path.resolve(here, 'frame.html'),
          hud: path.resolve(here, 'hud.html')
        }
      }
    }
  }
})
