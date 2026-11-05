import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { LEDGER } from './guard/consumers.js'
import { LfsPointerError } from './lfs.js'
import { committedLedger } from './orphan-drain.js'

const entry = { path: 'figma/review/src/generated/components/assets/a.svg', sha256: 'a'.repeat(64) }

// The library root (library/ui here) is a folder inside the repository, not its root (layout.ts).
describe('when reading the committed orphan ledger', () => {
  let repo: string
  beforeEach(async () => {
    repo = await mkdtemp(path.join(os.tmpdir(), 'committed-ledger-'))
    const git = (...args: string[]) =>
      execFileSync('git', [
        '-C',
        repo,
        '-c',
        'user.name=t',
        '-c',
        'user.email=t@example.com',
        ...args
      ])
    git('init', '-q')
    await mkdir(path.join(repo, 'library/ui', path.dirname(LEDGER)), { recursive: true })
    await writeFile(
      path.join(repo, 'library/ui', LEDGER),
      JSON.stringify({ schema: 1, orphaned: [entry] })
    )
    git('add', '-A')
    git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'ledger')
  })
  afterEach(async () => {
    await rm(repo, { recursive: true, force: true })
  })

  it('should find it relative to the library root', async () => {
    expect((await committedLedger(path.join(repo, 'library/ui'))).orphaned).toEqual([entry])
  })

  it('should read the ledger through the checkout filter that stores it', async () => {
    const git = (...args: string[]) => execFileSync('git', ['-C', repo, ...args])
    // A stand-in for Git LFS: the committed blob differs from the working file until smudged.
    git('config', 'filter.encoded.clean', 'base64')
    git('config', 'filter.encoded.smudge', 'base64 --decode')
    await writeFile(path.join(repo, '.gitattributes'), `library/ui/${LEDGER} filter=encoded\n`)
    await writeFile(
      path.join(repo, 'library/ui', LEDGER),
      JSON.stringify({ schema: 1, orphaned: [entry, entry] })
    )
    git('add', '-A')
    git(
      '-c',
      'user.name=t',
      '-c',
      'user.email=t@example.com',
      '-c',
      'commit.gpgsign=false',
      'commit',
      '-qm',
      'filtered'
    )

    expect((await committedLedger(path.join(repo, 'library/ui'))).orphaned).toEqual([entry, entry])
  })

  it('should fail loudly on an LFS pointer instead of reading an empty ledger', async () => {
    const git = (...args: string[]) => execFileSync('git', ['-C', repo, ...args])
    await writeFile(
      path.join(repo, 'library/ui', LEDGER),
      'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'b'.repeat(64) + '\nsize 1\n'
    )
    git('add', '-A')
    git(
      '-c',
      'user.name=t',
      '-c',
      'user.email=t@example.com',
      '-c',
      'commit.gpgsign=false',
      'commit',
      '-qm',
      'pointer'
    )

    await expect(committedLedger(path.join(repo, 'library/ui'))).rejects.toThrow(LfsPointerError)
  })

  it('should read nothing outside a git checkout', async () => {
    const outside = await mkdtemp(path.join(os.tmpdir(), 'no-git-'))
    try {
      expect((await committedLedger(outside)).orphaned).toEqual([])
    } finally {
      await rm(outside, { recursive: true, force: true })
    }
  })
})
