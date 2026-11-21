#!/usr/bin/env node
// The digest of a hosted gallery build (`vite build --mode hosted`): sha256 over one `<sha256>  <path>` line per
// file, sorted by path. gallery/hosted-build.sha256 records it. The Shared UI library run checks its build against
// the record before uploading it in the scanned Review artifact, and the Nix package Sites serves
// (infra/packages/ui-gallery.nix) checks its own build against the same record, so Sites ships the bytes the key
// scan read. Only regular files count: a link, or anything else, fails.
//
//   node gallery/scripts/hosted-digest.mjs <dir>                  prints the digest
//   node gallery/scripts/hosted-digest.mjs <dir> --check <file>   fails unless <file> holds it
//   node gallery/scripts/hosted-digest.mjs <dir> --write <file>   records it
import { createHash } from 'node:crypto'
import { lstatSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import process from 'node:process'

export function hostedDigest(root) {
  const lines = []
  const walk = (relative) => {
    for (const name of readdirSync(path.join(root, relative)).sort()) {
      const entry = relative ? `${relative}/${name}` : name
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

function main(argv) {
  const [dir, mode, file] = argv
  if (!dir || (mode && !['--check', '--write'].includes(mode)) || (mode && !file)) {
    console.error('usage: hosted-digest.mjs <dir> [--check <file> | --write <file>]')
    return 2
  }
  const digest = hostedDigest(dir)
  if (mode === '--write') writeFileSync(file, `${digest}\n`)
  else if (mode === '--check') {
    const recorded = readFileSync(file, 'utf8').trim()
    if (recorded !== digest) {
      console.error(
        `The hosted gallery build is ${digest}, but ${file} records ${recorded || 'nothing'}. ` +
          'From library/ui: pnpm exec vite build --config gallery/vite.config.ts --mode hosted && ' +
          'node gallery/scripts/hosted-digest.mjs gallery/dist --write gallery/hosted-build.sha256'
      )
      return 1
    }
  }
  console.log(digest)
  return 0
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(import.meta.filename))
  process.exitCode = main(process.argv.slice(2))
