import { mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

// One scratch folder per test run for fixtures that workers build once and copy per test.
// Workers inherit the variable; the folder is removed when the run ends, even if a worker was killed.
export default async function setup() {
  const folder = await mkdtemp(path.join(os.tmpdir(), 'dcl-mcp-figma-fixtures-'))
  process.env.DCL_FIXTURE_TEMPLATES = folder
  return async () => rm(folder, { recursive: true, force: true })
}
