#!/usr/bin/env node
// The digest of a hosted gallery build (`vite build --mode hosted`): sha256 over one `<sha256>  <path>` line per
// file, sorted by path. The Shared UI library run prints it for the build it puts in the scanned Review artifact as
// ui-gallery/, and the Nix package Sites serves (infra/packages/ui-gallery.nix) prints it for its own build; before
// an image is published, tooling/ui-gallery-receipt.py computes the same digest on both sides and requires a
// successful key scan of identical bytes. Only regular files count, and a hidden path fails: the artifact upload drops
// hidden files, so one would ship without being scanned.
//
//   node gallery/scripts/hosted-digest.mjs <dir>
import { createHash } from 'node:crypto'
import { lstatSync, readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import process from 'node:process'

export function hostedDigest(root) {
  const lines = []
  const walk = (relative) => {
    for (const name of readdirSync(path.join(root, relative)).sort()) {
      const entry = relative ? `${relative}/${name}` : name
      if (name.startsWith('.'))
        throw new Error(
          `${entry} is hidden: the artifact upload drops it, so it would ship unscanned`
        )
      const stat = lstatSync(path.join(root, entry))
      if (stat.isDirectory()) walk(entry)
      else if (stat.isFile())
        lines.push(
          `${createHash('sha256')
            .update(readFileSync(path.join(root, entry)))
            .digest('hex')}  ${entry}`
        )
      else throw new Error(`${entry} is not a regular file`)
    }
  }
  walk('')
  if (!lines.some((line) => line.endsWith('  index.html')))
    throw new Error(`${root} holds no index.html`)
  lines.sort((a, b) => (a.slice(66) < b.slice(66) ? -1 : 1))
  return createHash('sha256')
    .update(lines.map((line) => `${line}\n`).join(''))
    .digest('hex')
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(import.meta.filename)) {
  if (process.argv.length !== 3) {
    console.error('usage: hosted-digest.mjs <dir>')
    process.exitCode = 2
  } else console.log(hostedDigest(process.argv[2]))
}
