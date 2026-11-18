import { spawn, type ChildProcess } from 'node:child_process'
import { resolve } from 'node:path'
import { BASE, hermeticViteEnv } from './helpers/env'

let child: ChildProcess | undefined

/** Any HTTP answer, error statuses included, means something already holds the port. */
async function isUp(): Promise<boolean> {
  try {
    await fetch(BASE)
    return true
  } catch {
    return false
  }
}

async function waitForServer(server: ChildProcess, output: () => string, timeoutMs = 90_000): Promise<void> {
  let exited: Error | undefined
  server.once('error', error => (exited = error))
  server.once('exit', code => (exited = new Error(`Vite exited with code ${code}:\n${output()}`)))
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (exited) throw exited
    if (await isUp()) return
    await new Promise(r => setTimeout(r, 500))
  }
  throw new Error(`e2e dev server did not start on ${BASE} within ${timeoutMs}ms:\n${output()}`)
}

export async function setup(): Promise<void> {
  // Something already answering here (another worktree's server) would be tested instead of this code.
  if (await isUp()) {
    throw new Error(`${BASE} is already in use. Stop that server or run with E2E_PORT=<free port>.`)
  }
  const port = new URL(BASE).port
  child = spawn(resolve(process.cwd(), 'node_modules/.bin/vite'), ['--port', port, '--strictPort'], {
    cwd: process.cwd(),
    stdio: ['ignore', 'ignore', 'pipe'],
    env: hermeticViteEnv()
  })
  let stderr = ''
  child.stderr?.on('data', (chunk: Buffer) => (stderr = (stderr + chunk.toString()).slice(-4000)))
  await waitForServer(child, () => stderr)
}

export function teardown(): void {
  if (child && !child.killed) child.kill('SIGTERM')
}
