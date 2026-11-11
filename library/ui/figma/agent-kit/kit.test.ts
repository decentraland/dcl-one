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
  })

  describe('when the installed kit is newer than the library being installed', () => {
    it('should refuse the downgrade unless forced', async () => {
      const target = await temp('consumer-')
      await installKit({ target, kitRoot: await kitAtVersion(newerVersion) })

      await expect(installKit({ target })).rejects.toThrow(/newer than the kit/)
      await expect(installKit({ target, force: true })).resolves.toBeDefined()
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

  describe('when .agents/skills is the kit link', () => {
    it('should pass', async () => {
      const target = await temp('consumer-')
      await installKit({ target, linkAgents: true })

      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
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
