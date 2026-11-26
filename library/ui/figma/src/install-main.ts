import { realpathSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'
import { createApp } from './app-context.js'
import { runInstall } from './install.js'
import { errorText } from './server-errors.js'

// The process `dcl-figma install` runs in. components/generation.json binds the static import closure of this
// entry, so the install process loads no module outside that record; the CLI only forwards its arguments here.

// The install process may load only the tsx loader besides this closure: any other preload, condition or loader
// flag, NODE_OPTIONS, or a tsconfig other than the bound figma/tsconfig.json could load or alias code the record
// does not bind. `dcl-figma install` starts it that way; anything else is refused before installing.
function assertBoundProcess() {
  const tsx = path.dirname(createRequire(import.meta.url).resolve('tsx/package.json'))
  const loaders = [path.join(tsx, 'dist', 'preflight.cjs'), path.join(tsx, 'dist', 'loader.mjs')]
  const flags = process.execArgv
  const loaderOnly =
    flags.length % 2 === 0 &&
    flags.every((flag, index) => {
      if (index % 2) return true
      if (flag !== '--require' && flag !== '--import') return false
      const value = flags[index + 1]!
      if (/^[a-z][a-z0-9+.-]+:/i.test(value) && !value.startsWith('file:')) return false
      try {
        const file = value.startsWith('file:') ? fileURLToPath(value) : path.resolve(value)
        return loaders.includes(realpathSync(file))
      } catch {
        return false
      }
    })
  if (!loaderOnly)
    throw new Error('The install process runs only with the tsx loader: ' + flags.join(' '))
  if (process.env.NODE_OPTIONS) throw new Error('The install process runs without NODE_OPTIONS')
  if (process.env.TSX_TSCONFIG_PATH !== path.resolve(import.meta.dirname, '..', 'tsconfig.json'))
    throw new Error('The install process resolves only through library/ui/figma/tsconfig.json')
}

async function main(argv: string[]): Promise<number> {
  assertBoundProcess()
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    strict: true,
    options: { source: { type: 'string' }, out: { type: 'string' }, config: { type: 'string' } }
  })
  if (positionals.length !== 1) throw new Error('install takes foundations or components')
  if (values.source !== undefined && values.source !== 'snapshot' && values.source !== 'rest')
    throw new Error(`Unknown --source "${values.source}". Use snapshot or rest.`)
  const result = await runInstall(await createApp(process.cwd(), values.config), {
    target: positionals[0]!,
    source: values.source,
    outDir: values.out
  })
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
  return 0
}

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (error: unknown) => {
    process.stderr.write(`${errorText(error)}\n`)
    process.exit(1)
  }
)
