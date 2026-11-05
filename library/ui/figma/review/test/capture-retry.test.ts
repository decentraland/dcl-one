import assert from 'node:assert/strict'
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  readlink,
  rm,
  symlink,
  writeFile
} from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, it } from 'node:test'
import {
  CaptureTimeout,
  assertNodeVersion,
  captureWithRetry,
  createRetryBudget,
  pendingCommit,
  recoverInterruptedCommit,
  withStagedOutput
} from '../scripts/capture-retry.mjs'

const attempts = (...outcomes: (string | Error)[]) => {
  const calls = { count: 0 }
  const attempt = async () => {
    const outcome = outcomes[calls.count++]
    if (outcome instanceof Error) throw outcome
    return outcome
  }
  return { attempt, calls }
}
const options = () => {
  const lines: string[] = []
  const restarts = { count: 0 }
  return {
    lines,
    restarts,
    budget: createRetryBudget(),
    restart: async () => {
      restarts.count++
    },
    log: (line: string) => lines.push(line)
  }
}

describe('when a variant capture times out', () => {
  it('should retry once in a new browser and keep the second result', async () => {
    const { attempt, calls } = attempts(new CaptureTimeout('Preview did not become ready'), 'ok')
    const run = options()

    assert.equal(await captureWithRetry('Spinner-3', attempt, run), 'ok')
    assert.equal(calls.count, 2)
    assert.equal(run.restarts.count, 1)
    assert.deepEqual(run.lines, [
      'Spinner-3: Preview did not become ready; retrying once in a new browser'
    ])
  })

  it('should fail closed naming the variant when the retry times out too', async () => {
    const { attempt, calls } = attempts(
      new CaptureTimeout('CDP timeout: Runtime.evaluate'),
      new CaptureTimeout('CDP timeout: Runtime.evaluate')
    )

    await assert.rejects(captureWithRetry('Button-4', attempt, options()), {
      message: 'Button-4: capture timed out again after a retry: CDP timeout: Runtime.evaluate'
    })
    assert.equal(calls.count, 2)
  })

  it('should restart the browser only once per run', async () => {
    const run = options()
    for (const key of ['First-0', 'Second-0']) {
      const { attempt } = attempts(new CaptureTimeout('Preview did not become ready'), 'ok')
      await captureWithRetry(key, attempt, run)
    }

    assert.equal(run.restarts.count, 1)
    assert.deepEqual(run.lines, [
      'First-0: Preview did not become ready; retrying once in a new browser',
      'Second-0: Preview did not become ready; retrying once'
    ])
  })
})

describe('when a variant capture fails for another reason', () => {
  for (const [what, error] of [
    ['a page exception', new Error('Spinner-3: TypeError: x is undefined')],
    ['a render of the wrong size', new Error('Spinner-3: expected 40x40, rendered 0x0')]
  ] as const)
    it(`should not retry ${what}`, async () => {
      const { attempt, calls } = attempts(error, 'ok')
      const run = options()

      await assert.rejects(captureWithRetry('Spinner-3', attempt, run), error)
      assert.equal(calls.count, 1)
      assert.equal(run.restarts.count, 0)
      assert.deepEqual(run.lines, [])
    })
})

describe('when publishing the evidence of a capture run', () => {
  let root: string
  let output: string
  const OLD = {
    'fidelity.json': 'old evidence',
    'renders/Frozen-0.png': 'frozen render',
    'renders/Spinner-3.png': 'old render'
  }
  const NEW = {
    'fidelity.json': 'new evidence',
    'renders/Frozen-0.png': 'frozen render',
    'renders/Spinner-3.png': 'new render'
  }
  const live = async () =>
    Object.fromEntries(
      await Promise.all(
        Object.keys(OLD).map(
          async (file) => [file, await readFile(path.join(output, file), 'utf8')] as const
        )
      )
    )
  const leftovers = async () => (await readdir(root)).filter((entry) => entry !== 'public')
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'capture-run-'))
    output = path.join(root, 'public/dcl-ui-review')
    await mkdir(path.join(output, 'renders'), { recursive: true })
    for (const [file, bytes] of Object.entries(OLD)) await writeFile(path.join(output, file), bytes)
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  const capture = async (staging: string) => {
    await writeFile(path.join(staging, 'renders/Spinner-3.png'), 'new render')
    await writeFile(path.join(staging, 'fidelity.json'), 'new evidence')
  }

  it('should replace the pair and keep frozen renders when the run finishes', async () => {
    await withStagedOutput({ output, scratch: root }, capture)

    assert.deepEqual(await live(), NEW)
    assert.deepEqual(await leftovers(), [])
    assert.equal(await pendingCommit(root), false)
  })

  it('should leave the existing bytes untouched when the run fails before publishing', async () => {
    await assert.rejects(
      withStagedOutput({ output, scratch: root }, async (staging) => {
        await capture(staging)
        throw new Error('Button-4: capture timed out again after a retry')
      }),
      /timed out again/
    )

    assert.deepEqual(await live(), OLD)
    assert.deepEqual(await leftovers(), [])
  })

  for (const step of [
    'journal',
    'backup renders',
    'publish renders',
    'backup fidelity.json',
    'publish fidelity.json',
    'cleanup'
  ])
    it(`should complete the new pair on the next run after an interruption at ${step}`, async () => {
      await assert.rejects(
        withStagedOutput({ output, scratch: root }, capture, async (at) => {
          if (at === step) throw new Error('interrupted at ' + at)
        }),
        { message: 'interrupted at ' + step }
      )
      assert.equal(await pendingCommit(root), true)

      await recoverInterruptedCommit({ output, scratch: root })

      assert.deepEqual(await live(), NEW)
      assert.deepEqual(await leftovers(), [])
    })

  it('should also complete an interrupted commit when the recovery itself is interrupted once', async () => {
    await assert.rejects(
      withStagedOutput({ output, scratch: root }, capture, async (at) => {
        if (at === 'publish renders') throw new Error('first')
      })
    )
    await assert.rejects(
      recoverInterruptedCommit({ output, scratch: root }, async (at) => {
        if (at === 'publish fidelity.json') throw new Error('second')
      })
    )

    await recoverInterruptedCommit({ output, scratch: root })

    assert.deepEqual(await live(), NEW)
    assert.deepEqual(await leftovers(), [])
  })

  it('should complete a pending commit before starting the next run', async () => {
    await assert.rejects(
      withStagedOutput({ output, scratch: root }, capture, async (at) => {
        if (at === 'backup fidelity.json') throw new Error('interrupted')
      })
    )

    await withStagedOutput({ output, scratch: root }, async (staging) => {
      assert.equal(
        await readFile(path.join(staging, 'renders/Spinner-3.png'), 'utf8'),
        'new render'
      )
      await writeFile(path.join(staging, 'fidelity.json'), 'newer evidence')
    })

    assert.deepEqual(await live(), { ...NEW, 'fidelity.json': 'newer evidence' })
    assert.deepEqual(await leftovers(), [])
  })
})

describe('when the commit journal cannot be trusted', () => {
  let root: string
  let scratch: string
  let output: string
  // Every file and link under the workspace, so a refused recovery can be shown to change nothing.
  const snapshot = async (folder = root): Promise<Record<string, string>> => {
    const entries: Record<string, string> = {}
    for (const entry of await readdir(folder, { withFileTypes: true })) {
      const file = path.join(folder, entry.name)
      const key = path.relative(root, file)
      if (entry.isSymbolicLink()) entries[key] = 'link -> ' + (await readlink(file))
      else if (entry.isDirectory()) Object.assign(entries, { [key]: 'dir' }, await snapshot(file))
      else entries[key] = await readFile(file, 'utf8')
    }
    return entries
  }
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'capture-journal-'))
    scratch = path.join(root, 'review')
    output = path.join(scratch, 'public/dcl-ui-review')
    await mkdir(path.join(root, '.git'), { recursive: true })
    await writeFile(path.join(root, '.git/HEAD'), 'ref: refs/heads/main\n')
    await mkdir(path.join(output, 'renders'), { recursive: true })
    await writeFile(path.join(output, 'fidelity.json'), 'live evidence')
    await writeFile(path.join(output, 'renders/Spinner-3.png'), 'live render')
    await mkdir(path.join(scratch, '.capture-staging-ok/renders'), { recursive: true })
    await mkdir(path.join(scratch, 'staging-x'), { recursive: true })
    await writeFile(path.join(scratch, 'staging-x/keep'), 'keep')
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  const journal = (text: string) => writeFile(path.join(scratch, '.capture-commit.json'), text)

  for (const [what, prepare] of [
    ['a parent path', () => journal('{"staging":"../.git"}')],
    ['an absolute path', () => journal(JSON.stringify({ staging: path.join(root, '.git') }))],
    ['a nested path', () => journal('{"staging":"a/b"}')],
    ['a wrong prefix', () => journal('{"staging":"staging-x"}')],
    ['malformed JSON', () => journal('{"staging":')],
    ['an extra field', () => journal('{"staging":".capture-staging-ok","extra":1}')],
    [
      'a staging folder that is a symlink',
      async () => {
        await symlink(path.join(root, '.git'), path.join(scratch, '.capture-staging-link'))
        await journal('{"staging":".capture-staging-link"}')
      }
    ],
    [
      'a backup folder that is a symlink',
      async () => {
        await symlink(path.join(root, '.git'), path.join(scratch, '.capture-backup'))
        await journal('{"staging":".capture-staging-ok"}')
      }
    ]
  ] as const)
    it(`should refuse ${what} without changing anything`, async () => {
      await prepare()
      const before = await snapshot()

      await assert.rejects(
        recoverInterruptedCommit({ output, scratch }),
        /Refusing the capture commit/
      )
      await assert.rejects(
        withStagedOutput({ output, scratch }, async () => {}),
        /Refusing/
      )

      assert.deepEqual(await snapshot(), before)
      assert.equal(await pendingCommit(scratch), true)
    })

  it('should still complete a valid journal', async () => {
    await writeFile(path.join(scratch, '.capture-staging-ok/renders/Spinner-3.png'), 'new render')
    await writeFile(path.join(scratch, '.capture-staging-ok/fidelity.json'), 'new evidence')
    await journal('{"staging":".capture-staging-ok"}')

    await recoverInterruptedCommit({ output, scratch })

    assert.equal(await readFile(path.join(output, 'fidelity.json'), 'utf8'), 'new evidence')
    assert.equal(await readFile(path.join(output, 'renders/Spinner-3.png'), 'utf8'), 'new render')
    assert.equal(await readFile(path.join(root, '.git/HEAD'), 'utf8'), 'ref: refs/heads/main\n')
    assert.equal(await pendingCommit(scratch), false)
  })
})

describe('when checking the Node version', () => {
  it('should refuse a Node older than engines.node before the capture starts', () => {
    assert.throws(() => assertNodeVersion('22.23.4\n', '20.19.5'), {
      message: 'review:capture needs Node 22.23.4 (from engines.node); this is Node 20.19.5'
    })
  })

  it('should accept the same major or newer', () => {
    assertNodeVersion('22.23.4\n', '22.1.0')
    assertNodeVersion('22.23.4', '24.0.0')
  })
})
