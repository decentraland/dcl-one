// Whether this page offers the scene editor. The rest (the package, its services, the project the
// entry url names) is read in the Create page's own chunk (source.ts), so it costs the HUD nothing.

import { createContext } from 'react'
import type { EditorPin, EditorRelease } from './source'

/** The released package the page follows (README "Releasing the editor") and the hosts besides
 *  loopback that offer it, each only to an entry url with `editor-features=experimental`; null =
 *  none, so a production build carries no editor at all. */
export const RELEASED_EDITOR = {
  package: '@dcl-regenesislabs/web-editor',
  tag: 'latest',
  hosts: ['decentraland.zone', 'decentraland.org']
} as (EditorRelease & { hosts: string[] }) | null

/** Hosts that run one reviewed version instead of the tag: `editor.js` runs in this page, with the
 *  player's identity, so production checks it against a hash and ignores `?editor-tag`. Printed by
 *  the editor repo's `node packages/web/scripts/pin.mjs <version> <host...>`; a new version is a PR here. */
export const PINNED_EDITOR = {
  base: 'https://cdn.jsdelivr.net/npm/@dcl-regenesislabs/web-editor@0.3.27/',
  editorJsIntegrity: 'sha384-u1wBe2wxlnt7eJwk0SGrXXwLQjxtqTd3IKj/G8rQQsHilTzfJ/MOsZ07t3SzwpXw',
  editorSceneEntity: 'bafkreiggnrw7rgpjgljpbnvnppvxysckstguvp4s7zf65nfsm34vsenit4',
  hosts: ['decentraland.org']
} as (EditorPin & { hosts: string[] }) | null

// false folds every Create entry point out of a production build that has no release
export const EDITOR_BUILD = import.meta.env.DEV || RELEASED_EDITOR != null

export const isLoopback = (hostname: string): boolean => hostname === 'localhost' || hostname === '127.0.0.1'

/** `?editor-features=<a,b>`: what the entry url turns on. `experimental` offers the editor on a
 *  released host; `advanced` lets the editor show its whole machinery beside the assistant. A link
 *  can set either, so neither is a boundary: they keep the editor out of sight, nothing more. */
export const FEATURES_PARAM = 'editor-features'
export interface EditorFeatures {
  experimental: boolean
  advanced: boolean
}
export function editorFeatures(search: string): EditorFeatures {
  const named = new Set(
    new URLSearchParams(search)
      .getAll(FEATURES_PARAM)
      .flatMap((value) => value.split(','))
      .map((name) => name.trim().toLowerCase())
  )
  return { experimental: named.has('experimental'), advanced: named.has('advanced') }
}

/** `search`: the ENTRY url's query. */
export const editorOffered = (hostname: string, search: string): boolean =>
  EDITOR_BUILD &&
  (isLoopback(hostname) || ((RELEASED_EDITOR?.hosts.includes(hostname) ?? false) && editorFeatures(search).experimental))

/** Whether the Create entry points (menu top bar, sidebar rail) are offered. */
export const EditorOffered = createContext(false)
