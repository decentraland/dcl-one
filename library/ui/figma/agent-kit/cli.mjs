#!/usr/bin/env node
import path from 'node:path'
import process from 'node:process'
import { parseArgs } from 'node:util'
import { checkKit, installKit } from './kit.mjs'

const USAGE = `Usage:
  figma-agent-kit install [--target <repo>] [--force] [--agents-link]
  figma-agent-kit check [--target <repo>]
`

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    target: { type: 'string' },
    force: { type: 'boolean', default: false },
    'agents-link': { type: 'boolean', default: false },
    // Accepted for older scripts; the link is off unless --agents-link asks for it.
    'no-agents-link': { type: 'boolean', default: false }
  }
})
const target = path.resolve(values.target ?? process.cwd())

try {
  if (positionals[0] === 'install') {
    const result = await installKit({
      target,
      force: values.force,
      linkAgents: values['agents-link'] && !values['no-agents-link']
    })
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
  } else if (positionals[0] === 'check') {
    const result = await checkKit({ target })
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
    process.exitCode = result.ok ? 0 : 1
  } else {
    process.stderr.write(USAGE)
    process.exitCode = 2
  }
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
}
