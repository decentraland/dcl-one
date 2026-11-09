import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { writeFile } from 'node:fs/promises'
import path from 'node:path'

const execute = promisify(execFile)
// Synthetic test history must not depend on a developer's signing key or global Git hooks/config.
const fixtureEnvironment = {
  ...process.env,
  GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_CONFIG_NOSYSTEM: '1',
  GIT_AUTHOR_NAME: 'Fixture',
  GIT_AUTHOR_EMAIL: 'fixture@example.invalid',
  GIT_COMMITTER_NAME: 'Fixture',
  GIT_COMMITTER_EMAIL: 'fixture@example.invalid'
}

const fixtureGit = async (root: string, ...args: string[]) =>
  (await execute('git', ['-C', root, ...args], { env: fixtureEnvironment })).stdout.trim()

async function fixtureCommit(root: string, message = 'test: record fixture') {
  await fixtureGit(root, 'add', '.')
  const tree = await fixtureGit(root, 'write-tree')
  const parent = await fixtureGit(root, 'rev-parse', '--verify', 'HEAD').catch(() => undefined)
  const commit = await fixtureGit(
    root,
    'commit-tree',
    tree,
    ...(parent ? ['-p', parent] : []),
    '-m',
    message
  )
  await fixtureGit(root, 'update-ref', 'HEAD', commit)
  return commit
}

async function initializeFixtureGit(root: string) {
  await writeFile(path.join(root, '.gitignore'), '.figma-cache\n')
  await fixtureGit(root, 'init', '--quiet')
  await fixtureGit(root, 'symbolic-ref', 'HEAD', 'refs/heads/fixture-release')
  const base = await fixtureCommit(root, 'test: record release base')
  await fixtureGit(root, 'update-ref', 'refs/remotes/origin/main', base)
  return base
}

export { fixtureGit, fixtureCommit, initializeFixtureGit }
