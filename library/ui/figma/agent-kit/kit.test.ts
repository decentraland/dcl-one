import { createHash } from 'node:crypto'
import {
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  readlink,
  rm,
  stat,
  symlink,
  writeFile
} from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { checkKit, installKit } from './kit.mjs'

const packageVersion = JSON.parse(
  await readFile(new URL('../../package.json', import.meta.url), 'utf8')
).version as string
const newerVersion = `${Number(packageVersion.split('.')[0]) + 1}.0.0`
const created: string[] = []

async function temp(prefix: string) {
  const dir = await mkdtemp(path.join(tmpdir(), prefix))
  created.push(dir)
  return dir
}

// A copy of the kit under a package.json of another version, to simulate an upgraded library.
async function kitAtVersion(version: string) {
  const root = await temp('figma-kit-')
  await cp(import.meta.dirname, path.join(root, 'figma/agent-kit'), { recursive: true })
  await writeFile(path.join(root, 'package.json'), JSON.stringify({ name: '@dcl/one-ui', version }))
  return path.join(root, 'figma/agent-kit')
}

afterEach(async () => {
  await Promise.all(created.splice(0).map((dir) => rm(dir, { recursive: true, force: true })))
})

describe('installKit', () => {
  describe('when installing into an empty repository', () => {
    it('should copy the skill, record its version and create no .agents link by default', async () => {
      const target = await temp('consumer-')

      const result = await installKit({ target })

      expect(result.installed).toEqual(['implement-figma-design', 'maintain-figma-library'])
      expect(result.agentsLink).toBe('skipped')
      expect(await lstat(path.join(target, '.agents')).catch(() => undefined)).toBeUndefined()
    })

    it('should link .agents/skills when asked', async () => {
      const target = await temp('consumer-')

      const result = await installKit({ target, linkAgents: true })

      expect(result.installed).toEqual(['implement-figma-design', 'maintain-figma-library'])
      expect(result.agentsLink).toBe('created')
      const manifest = JSON.parse(
        await readFile(path.join(target, '.claude/skills/implement-figma-design/.kit.json'), 'utf8')
      )
      expect(manifest).toMatchObject({ library: '@dcl/one-ui', version: packageVersion })
      expect(await readlink(path.join(target, '.agents/skills'))).toBe('../.claude/skills')
    })
  })

  describe('when a skill with the same name exists and was not installed by the kit', () => {
    it('should refuse to overwrite it', async () => {
      const target = await temp('consumer-')
      const own = path.join(target, '.claude/skills/implement-figma-design')
      await mkdir(own, { recursive: true })
      await writeFile(path.join(own, 'SKILL.md'), 'mine')

      await expect(installKit({ target })).rejects.toThrow(/not installed by this kit/)
      expect(await readFile(path.join(own, 'SKILL.md'), 'utf8')).toBe('mine')
    })

    it('should refuse before installing any other skill', async () => {
      const target = await temp('consumer-')
      const own = path.join(target, '.claude/skills/maintain-figma-library')
      await mkdir(own, { recursive: true })
      await writeFile(path.join(own, 'SKILL.md'), 'mine')

      await expect(installKit({ target })).rejects.toThrow(/not installed by this kit/)
      expect(await readdir(path.join(target, '.claude/skills'))).toEqual(['maintain-figma-library'])
    })

    it('should replace it with --force', async () => {
      const target = await temp('consumer-')
      const own = path.join(target, '.claude/skills/implement-figma-design')
      await mkdir(own, { recursive: true })
      await writeFile(path.join(own, 'mine.md'), 'mine')

      const result = await installKit({ target, force: true })

      expect(result.installed).toContain('implement-figma-design')
      expect(await lstat(path.join(own, 'mine.md')).catch(() => undefined)).toBeUndefined()
      expect(await checkKit({ target })).toMatchObject({ ok: true })
    })
  })

  describe('when an installed manifest is not valid JSON', () => {
    it('should refuse without --force, naming the file, and repair it with --force', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      const manifest = path.join(target, '.claude/skills/implement-figma-design/.kit.json')
      await writeFile(manifest, '{"library": "@dcl/one')

      await expect(installKit({ target })).rejects.toThrow(
        /\.kit\.json is not a valid kit manifest/
      )
      expect(await readFile(manifest, 'utf8')).toBe('{"library": "@dcl/one')
      expect((await installKit({ target, force: true })).installed).toEqual([
        'implement-figma-design'
      ])
      expect(await checkKit({ target })).toMatchObject({ ok: true })
    })
  })

  describe.each(['.claude', '.claude/skills'])('when %s is a symlink', (linked) => {
    it('should refuse before writing through the link', async () => {
      const target = await temp('consumer-')
      const outside = await temp('outside-')
      await mkdir(path.dirname(path.join(target, linked)), { recursive: true })
      await symlink(outside, path.join(target, linked))

      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/is a link/)
      expect(await readdir(outside)).toEqual([])
      expect(await lstat(path.join(target, '.agents')).catch(() => undefined)).toBeUndefined()
    })
  })

  describe('when the installed kit is newer than the library being installed', () => {
    it('should refuse the downgrade unless forced', async () => {
      const target = await temp('consumer-')
      await installKit({ target, kitRoot: await kitAtVersion(newerVersion) })

      await expect(installKit({ target })).rejects.toThrow(/newer than the kit/)
      const result = await installKit({ target, force: true })
      expect(result.installed).toEqual(['implement-figma-design', 'maintain-figma-library'])
      expect(await checkKit({ target })).toMatchObject({ ok: true })
    })

    it.each([
      ['a prerelease of a newer version', `${newerVersion}-rc.1`, /newer than the kit/],
      ['a version that is not semver', 'next', /cannot be ordered/]
    ])('should refuse %s unless forced', async (_label, version, message) => {
      const target = await temp('consumer-')
      await installKit({ target, kitRoot: await kitAtVersion(version) })
      const before = await readFile(
        path.join(target, '.claude/skills/implement-figma-design/.kit.json'),
        'utf8'
      )

      await expect(installKit({ target })).rejects.toThrow(message)
      expect(
        await readFile(path.join(target, '.claude/skills/implement-figma-design/.kit.json'), 'utf8')
      ).toBe(before)
    })
  })

  describe('when the installed kit is a prerelease of this version', () => {
    it('should upgrade it without --force', async () => {
      const target = await temp('consumer-')
      await installKit({ target, kitRoot: await kitAtVersion(`${packageVersion}-rc.1`) })

      expect((await installKit({ target })).installed).toEqual([
        'implement-figma-design',
        'maintain-figma-library'
      ])
    })
  })

  describe('when reinstalling without --agents-link after a linked install', () => {
    it('should rewrite the manifests without the link and leave the link in place', async () => {
      const target = await temp('consumer-')
      await installKit({ target, linkAgents: true })

      const result = await installKit({ target })

      expect(result).toMatchObject({
        installed: ['implement-figma-design', 'maintain-figma-library'],
        agentsLink: 'skipped'
      })
      expect(await readlink(path.join(target, '.agents/skills'))).toBe('../.claude/skills')
      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
  })

  describe('when installing twice', () => {
    it('should leave the installed bytes and timestamps untouched the second time', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      const skill = path.join(target, '.claude/skills/implement-figma-design')
      // Every file below the skill, with its sha256 and modification time.
      const snapshot = async () =>
        Promise.all(
          (await readdir(skill, { recursive: true, withFileTypes: true }))
            .filter((entry) => entry.isFile())
            .map((entry) => path.join(entry.parentPath, entry.name))
            .sort()
            .map(async (full) => [
              path.relative(skill, full),
              createHash('sha256')
                .update(await readFile(full))
                .digest('hex'),
              (await stat(full)).mtimeMs
            ])
        )
      const before = await snapshot()

      const result = await installKit({ target })

      expect(result).toMatchObject({
        installed: [],
        unchanged: ['implement-figma-design', 'maintain-figma-library']
      })
      expect(await snapshot()).toEqual(before)
    })
  })

  describe('when .agents/skills is already the kit link', () => {
    it('should report it as already linked', async () => {
      const target = await temp('consumer-')
      await installKit({ target, linkAgents: true })

      expect((await installKit({ target, linkAgents: true })).agentsLink).toBe('already-linked')
    })
  })

  describe('when .agents/skills is a real directory', () => {
    it.each([false, true])(
      'should refuse before writing anything and keep the directory (force %s)',
      async (force) => {
        const target = await temp('consumer-')
        await mkdir(path.join(target, '.agents/skills'), { recursive: true })
        await writeFile(path.join(target, '.agents/skills/own.md'), 'mine')

        await expect(installKit({ target, linkAgents: true, force })).rejects.toThrow(
          /is not a link to/
        )

        expect(await readFile(path.join(target, '.agents/skills/own.md'), 'utf8')).toBe('mine')
        expect(await lstat(path.join(target, '.claude')).catch(() => undefined)).toBeUndefined()
      }
    )
  })

  describe('when .agents/skills links elsewhere', () => {
    it('should refuse without --force and leave the link', async () => {
      const target = await temp('consumer-')
      await mkdir(path.join(target, '.agents'))
      await symlink('../elsewhere', path.join(target, '.agents/skills'))

      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/links elsewhere/)
      expect(await readlink(path.join(target, '.agents/skills'))).toBe('../elsewhere')
    })

    it('should replace only the link with --force', async () => {
      const target = await temp('consumer-')
      await mkdir(path.join(target, 'elsewhere'))
      await writeFile(path.join(target, 'elsewhere/keep.md'), 'kept')
      await mkdir(path.join(target, '.agents'))
      await symlink('../elsewhere', path.join(target, '.agents/skills'))

      const result = await installKit({ target, linkAgents: true, force: true })

      expect(result.agentsLink).toBe('replaced')
      expect(await readlink(path.join(target, '.agents/skills'))).toBe('../.claude/skills')
      expect(await readFile(path.join(target, 'elsewhere/keep.md'), 'utf8')).toBe('kept')
    })
  })
})

describe('checkKit', () => {
  describe('when the installed skill matches the library', () => {
    it('should pass', async () => {
      const target = await temp('consumer-')
      await installKit({ target })

      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
  })

  describe('when the skill was never installed', () => {
    it('should report not-installed', async () => {
      const target = await temp('consumer-')

      const result = await checkKit({ target })

      expect(result.problems.map((p) => p.reason)).toEqual(['not-installed', 'not-installed'])
    })
  })

  describe('when the installed skill is newer than the library', () => {
    it('should report it as stale and name install --force, which install requires', async () => {
      const target = await temp('consumer-')
      await installKit({ target, kitRoot: await kitAtVersion(newerVersion) })

      const result = await checkKit({ target })

      expect(result.problems.map((p) => [p.reason, p.message.includes('install --force')])).toEqual(
        [
          ['stale', true],
          ['stale', true]
        ]
      )
    })
  })

  describe('when an installed manifest is not valid JSON', () => {
    it('should report it as modified instead of crashing', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      await writeFile(path.join(target, '.claude/skills/implement-figma-design/.kit.json'), 'oops')

      const result = await checkKit({ target })

      expect(result.problems).toEqual([
        expect.objectContaining({ skill: 'implement-figma-design', reason: 'modified' })
      ])
    })
  })

  describe('when a skill path is a file', () => {
    it('should report it as not installed instead of crashing', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      const skill = path.join(target, '.claude/skills/implement-figma-design')
      await rm(skill, { recursive: true })
      await writeFile(skill, 'not a directory')

      expect((await checkKit({ target })).problems).toEqual([
        expect.objectContaining({ skill: 'implement-figma-design', reason: 'not-installed' })
      ])
      await expect(installKit({ target })).rejects.toThrow(/is not a directory the kit installed/)
      expect((await installKit({ target, force: true })).installed).toEqual([
        'implement-figma-design'
      ])
    })
  })

  describe('when .claude is a symlink', () => {
    it('should fail without following it', async () => {
      const target = await temp('consumer-')
      const outside = await temp('outside-')
      await installKit({ target: outside })
      await symlink(path.join(outside, '.claude'), path.join(target, '.claude'))

      expect(await checkKit({ target })).toMatchObject({
        ok: false,
        problems: [{ skill: '.claude/skills', reason: 'not-installed' }]
      })
    })
  })

  describe('when an installed file has CRLF line endings', () => {
    it('should pass, as a Windows checkout with core.autocrlf would have it', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      const skill = path.join(target, '.claude/skills/implement-figma-design/SKILL.md')
      await writeFile(skill, (await readFile(skill, 'utf8')).replaceAll('\n', '\r\n'))

      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })

    it('should still report a change to the text itself', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      const skill = path.join(target, '.claude/skills/implement-figma-design/SKILL.md')
      await writeFile(skill, `${(await readFile(skill, 'utf8')).replaceAll('\n', '\r\n')}x`)

      expect((await checkKit({ target })).problems.map((p) => p.reason)).toEqual(['modified'])
    })
  })

  describe('when a file is renamed and its content shifted into the name', () => {
    it('should report it as modified', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      const dir = path.join(target, '.claude/skills/implement-figma-design')
      const content = await readFile(path.join(dir, 'SKILL.md'))
      await rm(path.join(dir, 'SKILL.md'))
      await writeFile(
        path.join(dir, `SKILL.md${content.subarray(0, 1).toString()}`),
        content.subarray(1)
      )

      expect((await checkKit({ target })).problems.map((p) => p.reason)).toEqual(['modified'])
    })
  })

  describe('when the library was upgraded after install', () => {
    it('should report the skill as stale', async () => {
      const target = await temp('consumer-')
      await installKit({ target })

      const result = await checkKit({ target, kitRoot: await kitAtVersion(newerVersion) })

      expect(result.problems.map((p) => p.reason)).toEqual(['stale', 'stale'])
    })
  })

  describe('when the installed skill was edited by hand', () => {
    it('should report it as modified', async () => {
      const target = await temp('consumer-')
      await installKit({ target })
      await writeFile(path.join(target, '.claude/skills/implement-figma-design/SKILL.md'), 'edited')

      const result = await checkKit({ target })

      expect(result.problems.map((p) => p.reason)).toEqual(['modified'])
    })
  })

  describe('when the kit source changes without a reinstall', () => {
    it('should report the installed skill as stale', async () => {
      const target = await temp('consumer-')
      const kitRoot = await kitAtVersion(packageVersion)
      await installKit({ target, kitRoot })
      await writeFile(path.join(kitRoot, 'skills/implement-figma-design/SKILL.md'), 'changed')

      const result = await checkKit({ target, kitRoot })

      expect(result.problems.map((p) => p.reason)).toEqual(['stale'])
    })
  })

  describe.each([
    ['a real directory', (link: string) => mkdir(link, { recursive: true })],
    [
      'a link elsewhere',
      async (link: string) => {
        await mkdir(path.dirname(link), { recursive: true })
        await symlink('../elsewhere', link)
      }
    ]
  ])('when .agents/skills is %s', (_label, make) => {
    it('should report agents-link-mismatch after an install that asked for the link', async () => {
      const target = await temp('consumer-')
      await installKit({ target, linkAgents: true })
      await rm(path.join(target, '.agents/skills'))
      await make(path.join(target, '.agents/skills'))

      const result = await checkKit({ target })

      expect(result).toMatchObject({ ok: false })
      expect(result.problems.map((p) => p.reason)).toEqual(['agents-link-mismatch'])
    })
  })

  describe('when the consumer owns .agents/skills and installed without the link', () => {
    it.each([
      ['a real directory', (link: string) => mkdir(link, { recursive: true })],
      [
        'a link elsewhere',
        async (link: string) => {
          await mkdir(path.dirname(link), { recursive: true })
          await symlink('../elsewhere', link)
        }
      ]
    ])('should pass with %s', async (_label, make) => {
      const target = await temp('consumer-')
      await make(path.join(target, '.agents/skills'))
      await installKit({ target })

      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
  })

  describe('when the kit link is spelled differently but resolves to the skills', () => {
    it.each(['./../.claude/skills', '../.claude/skills/'])('should accept %s', async (spelling) => {
      const target = await temp('consumer-')
      await installKit({ target, linkAgents: true })
      await rm(path.join(target, '.agents/skills'))
      await symlink(spelling, path.join(target, '.agents/skills'))

      expect((await installKit({ target, linkAgents: true })).agentsLink).toBe('already-linked')
      expect(await checkKit({ target })).toMatchObject({ ok: true })
    })
  })

  describe.each([
    [
      'a symlink to another directory',
      async (target: string) => {
        await mkdir(path.join(target, 'outside'))
        await symlink('outside', path.join(target, '.agents'))
      }
    ],
    ['a file', (target: string) => writeFile(path.join(target, '.agents'), 'not a directory')]
  ])('when .agents itself is %s', (_label, make) => {
    it('should refuse the link instead of writing outside the target', async () => {
      const target = await temp('consumer-')
      await make(target)

      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/is not a link to/)
      expect(
        await lstat(path.join(target, 'outside/skills')).catch(() => undefined)
      ).toBeUndefined()
    })
  })

  describe('when .agents/skills is a broken symlink', () => {
    it('should refuse it without --force and replace it with --force', async () => {
      const target = await temp('consumer-')
      await mkdir(path.join(target, '.agents'))
      await symlink('../missing', path.join(target, '.agents/skills'))

      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/links elsewhere/)
      expect(await lstat(path.join(target, '.claude')).catch(() => undefined)).toBeUndefined()
      expect((await installKit({ target, linkAgents: true, force: true })).agentsLink).toBe(
        'replaced'
      )
    })
  })

  describe('when .agents/skills is the kit link', () => {
    it('should pass', async () => {
      const target = await temp('consumer-')
      await installKit({ target, linkAgents: true })

      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
  })
})

describe('the figma-agent-kit command', () => {
  const cli = (args: string[]) =>
    spawnSync(process.execPath, [path.join(import.meta.dirname, 'cli.mjs'), ...args], {
      encoding: 'utf8'
    })

  it('should let --no-agents-link override --agents-link', async () => {
    const target = await temp('consumer-')

    const run = cli(['install', '--target', target, '--agents-link', '--no-agents-link'])

    expect(run.status).toBe(0)
    expect(JSON.parse(run.stdout)).toMatchObject({ agentsLink: 'skipped' })
    expect(await lstat(path.join(target, '.agents')).catch(() => undefined)).toBeUndefined()
  })

  it('should exit 0 on a passing check and 1 on a failing one', async () => {
    const target = await temp('consumer-')

    expect(cli(['check', '--target', target]).status).toBe(1)
    expect(cli(['install', '--target', target]).status).toBe(0)
    expect(cli(['check', '--target', target]).status).toBe(0)
  })

  it('should exit 1 with the reason on stderr when install refuses', async () => {
    const target = await temp('consumer-')
    await mkdir(path.join(target, '.claude/skills/implement-figma-design'), { recursive: true })

    const run = cli(['install', '--target', target])

    expect([run.status, run.stdout]).toEqual([1, ''])
    expect(run.stderr).toMatch(/not installed by this kit/)
  })

  it.each([[[]], [['unknown']]])('should exit 2 with the usage for %j', (args) => {
    const run = cli(args)

    expect(run.status).toBe(2)
    expect(run.stderr).toMatch(/^Usage:/)
  })
})

// dcl-one installs its own skill at the repository root; CI runs figma-agent-kit check over it.
describe('the install committed in this repository', () => {
  const repository = path.resolve(import.meta.dirname, '../../../..')

  it('should match the kit', async () => {
    expect(await checkKit({ target: repository })).toMatchObject({ ok: true, problems: [] })
  })

  it('should carry no Figma URL', async () => {
    const skill = await readFile(
      path.join(repository, '.claude/skills/implement-figma-design/SKILL.md'),
      'utf8'
    )
    expect(skill).not.toMatch(/figma\.com\/(design|file|board|proto)/i)
  })
})
