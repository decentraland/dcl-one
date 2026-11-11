import { parseArgs } from 'node:util'
import { createApp } from './app-context.js'
import { runInstall } from './install.js'
import { errorText } from './server-errors.js'

// The process `dcl-figma install` runs in. components/generation.json binds the static import closure of this
// entry, so the install process loads no module outside that record; the CLI only forwards its arguments here.

async function main(argv: string[]): Promise<number> {
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
