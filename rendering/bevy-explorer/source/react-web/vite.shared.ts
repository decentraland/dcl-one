import { fileURLToPath } from 'node:url'

// Sources the HUD compiles from library/: the in-world Shop's logic and the catalyst's DTOs. library/ has
// no node_modules, so the packages they import resolve to this app's copies (`SHARED_DEDUPE`), and the
// bundle holds one viem, zod, React and React Query. tsconfig.json maps the same paths for the type check.
// tooling/play-artifact.py hashes these trees with the HUD (`HUD_SHARED`); a test keeps the lists together.
export const SHARED_SOURCES: Record<string, string> = {
  '@shop': fileURLToPath(new URL('../../../../library/shop', import.meta.url)),
  '@catalyst-api': fileURLToPath(new URL('../../../../library/catalyst-api', import.meta.url))
}

export const SHARED_DEDUPE = ['react', 'react-dom', 'viem', 'zod', '@tanstack/react-query']
