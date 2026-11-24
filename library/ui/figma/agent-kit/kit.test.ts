import { createHash } from 'node:crypto'
import {
  chmod,
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
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
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
async function kitAtVersion(version: string, name = '@dcl/one-ui') {
  const root = await temp('figma-kit-')
  await cp(import.meta.dirname, path.join(root, 'figma/agent-kit'), { recursive: true })
  await writeFile(path.join(root, 'package.json'), JSON.stringify({ name, version }))
  return path.join(root, 'figma/agent-kit')
}

afterEach(async () => {
  await Promise.all(created.splice(0).map((dir) => rm(dir, { recursive: true, force: true })))
})

describe('installKit', () => {
  let target: string
  beforeEach(async () => {
    target = await temp('consumer-')
  })

  describe('when installing into an empty repository', () => {
    describe('and no .agents link is asked for', () => {
      let result: Awaited<ReturnType<typeof installKit>>
      beforeEach(async () => {
        result = await installKit({ target })
      })

      it('should copy the skill, record its version and create no .agents link by default', async () => {
        expect(result.installed).toEqual(['implement-figma-design', 'maintain-figma-library'])
        expect(result.agentsLink).toBe('skipped')
        expect(await lstat(path.join(target, '.agents')).catch(() => undefined)).toBeUndefined()
      })
    })

    describe('and the .agents link is asked for', () => {
      let result: Awaited<ReturnType<typeof installKit>>
      beforeEach(async () => {
        result = await installKit({ target, linkAgents: true })
      })

      it('should link .agents/skills', async () => {
        expect(result.installed).toEqual(['implement-figma-design', 'maintain-figma-library'])
        expect(result.agentsLink).toBe('created')
        const manifest = JSON.parse(
          await readFile(
            path.join(target, '.claude/skills/implement-figma-design/.kit.json'),
            'utf8'
          )
        )
        expect(manifest).toMatchObject({ library: '@dcl/one-ui', version: packageVersion })
        expect(await readlink(path.join(target, '.agents/skills'))).toBe('../.claude/skills')
      })
    })
  })

  describe('when a skill with the same name exists and was not installed by the kit', () => {
    describe('and it is the first skill the kit installs', () => {
      let own: string
      beforeEach(async () => {
        own = path.join(target, '.claude/skills/implement-figma-design')
        await mkdir(own, { recursive: true })
        await writeFile(path.join(own, 'SKILL.md'), 'mine')
      })

      it('should refuse to overwrite it', async () => {
        await expect(installKit({ target })).rejects.toThrow(/not installed by this kit/)
        expect(await readFile(path.join(own, 'SKILL.md'), 'utf8')).toBe('mine')
      })
    })

    describe('and it is the last skill the kit installs', () => {
      beforeEach(async () => {
        const own = path.join(target, '.claude/skills/maintain-figma-library')
        await mkdir(own, { recursive: true })
        await writeFile(path.join(own, 'SKILL.md'), 'mine')
      })

      it('should refuse before installing any other skill', async () => {
        await expect(installKit({ target })).rejects.toThrow(/not installed by this kit/)
        expect(await readdir(path.join(target, '.claude/skills'))).toEqual([
          'maintain-figma-library'
        ])
      })
    })

    describe('and the install is forced', () => {
      let own: string
      beforeEach(async () => {
        own = path.join(target, '.claude/skills/implement-figma-design')
        await mkdir(own, { recursive: true })
        await writeFile(path.join(own, 'mine.md'), 'mine')
      })

      it('should replace it with --force', async () => {
        const result = await installKit({ target, force: true })

        expect(result.installed).toContain('implement-figma-design')
        expect(await lstat(path.join(own, 'mine.md')).catch(() => undefined)).toBeUndefined()
        expect(await checkKit({ target })).toMatchObject({ ok: true })
      })
    })
  })

  describe('when an installed manifest is not valid JSON', () => {
    let manifest: string
    beforeEach(async () => {
      await installKit({ target })
      manifest = path.join(target, '.claude/skills/implement-figma-design/.kit.json')
      await writeFile(manifest, '{"library": "@dcl/one')
    })

    it('should refuse without --force, naming the file, and repair it with --force', async () => {
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

  describe.each([
    ['unreadable', (file: string) => chmod(file, 0o000)],
    [
      'a directory',
      async (file: string) => {
        await rm(file)
        await mkdir(file)
      }
    ]
  ])('when an installed manifest is %s', (_label, make) => {
    beforeEach(async () => {
      await installKit({ target })
      await make(path.join(target, '.claude/skills/implement-figma-design/.kit.json'))
    })

    it('should refuse without --force, repair with --force, and be reported by check', async () => {
      expect((await checkKit({ target })).problems).toEqual([
        expect.objectContaining({ skill: 'implement-figma-design', reason: 'modified' })
      ])
      await expect(installKit({ target })).rejects.toThrow(/cannot be read/)
      expect((await installKit({ target, force: true })).installed).toEqual([
        'implement-figma-design'
      ])
      expect(await checkKit({ target })).toMatchObject({ ok: true })
    })
  })

  describe('when a skill was installed by another library kit at the same version', () => {
    beforeEach(async () => {
      await installKit({ target, kitRoot: await kitAtVersion(packageVersion, '@other/ui') })
    })

    it('should refuse without --force and report it in check', async () => {
      expect((await checkKit({ target })).problems.map((p) => p.message)).toEqual([
        expect.stringMatching(/installed by the kit of @other\/ui/),
        expect.stringMatching(/installed by the kit of @other\/ui/)
      ])
      await expect(installKit({ target })).rejects.toThrow(/installed by the kit of @other\/ui/)
      expect((await installKit({ target, force: true })).installed).toHaveLength(2)
    })
  })

  describe.each(['.claude', '.claude/skills'])('when %s is a symlink', (linked) => {
    let outside: string
    beforeEach(async () => {
      outside = await temp('outside-')
      await mkdir(path.dirname(path.join(target, linked)), { recursive: true })
      await symlink(outside, path.join(target, linked))
    })

    it('should refuse before writing through the link', async () => {
      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/is a link/)
      expect(await readdir(outside)).toEqual([])
      expect(await lstat(path.join(target, '.agents')).catch(() => undefined)).toBeUndefined()
    })
  })

  describe('when the installed kit is newer than the library being installed', () => {
    beforeEach(async () => {
      await installKit({ target, kitRoot: await kitAtVersion(newerVersion) })
    })

    it('should refuse the downgrade unless forced', async () => {
      await expect(installKit({ target })).rejects.toThrow(/newer than the kit/)
      const result = await installKit({ target, force: true })
      expect(result.installed).toEqual(['implement-figma-design', 'maintain-figma-library'])
      expect(await checkKit({ target })).toMatchObject({ ok: true })
    })
  })

  describe.each([
    ['a prerelease of a newer version', `${newerVersion}-rc.1`, /newer than the kit/],
    ['a version that is not semver', 'next', /cannot be ordered/]
  ])('when the installed kit is %s', (_label, version, message) => {
    let before: string
    beforeEach(async () => {
      await installKit({ target, kitRoot: await kitAtVersion(version) })
      before = await readFile(
        path.join(target, '.claude/skills/implement-figma-design/.kit.json'),
        'utf8'
      )
    })

    it('should refuse to replace it unless forced', async () => {
      await expect(installKit({ target })).rejects.toThrow(message)
      expect(
        await readFile(path.join(target, '.claude/skills/implement-figma-design/.kit.json'), 'utf8')
      ).toBe(before)
    })
  })

  describe('when the installed kit is a prerelease of this version', () => {
    beforeEach(async () => {
      await installKit({ target, kitRoot: await kitAtVersion(`${packageVersion}-rc.1`) })
    })

    it('should upgrade it without --force', async () => {
      expect((await installKit({ target })).installed).toEqual([
        'implement-figma-design',
        'maintain-figma-library'
      ])
    })
  })

  describe('when reinstalling without --agents-link after a linked install', () => {
    beforeEach(async () => {
      await installKit({ target, linkAgents: true })
    })

    it('should rewrite the manifests without the link and leave the link in place', async () => {
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
    let skill: string
    let before: unknown[]
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
    beforeEach(async () => {
      await installKit({ target })
      skill = path.join(target, '.claude/skills/implement-figma-design')
      before = await snapshot()
    })

    it('should leave the installed bytes and timestamps untouched the second time', async () => {
      const result = await installKit({ target })

      expect(result).toMatchObject({
        installed: [],
        unchanged: ['implement-figma-design', 'maintain-figma-library']
      })
      expect(await snapshot()).toEqual(before)
    })
  })

  describe('when .agents/skills is already the kit link', () => {
    beforeEach(async () => {
      await installKit({ target, linkAgents: true })
    })

    it('should report it as already linked', async () => {
      expect((await installKit({ target, linkAgents: true })).agentsLink).toBe('already-linked')
    })
  })

  describe('when .agents/skills is a real directory', () => {
    beforeEach(async () => {
      await mkdir(path.join(target, '.agents/skills'), { recursive: true })
      await writeFile(path.join(target, '.agents/skills/own.md'), 'mine')
    })

    it.each([false, true])(
      'should refuse before writing anything and keep the directory (force %s)',
      async (force) => {
        await expect(installKit({ target, linkAgents: true, force })).rejects.toThrow(
          /is not a link to/
        )

        expect(await readFile(path.join(target, '.agents/skills/own.md'), 'utf8')).toBe('mine')
        expect(await lstat(path.join(target, '.claude')).catch(() => undefined)).toBeUndefined()
      }
    )
  })

  describe('when .agents/skills links elsewhere', () => {
    describe('and the install is not forced', () => {
      beforeEach(async () => {
        await mkdir(path.join(target, '.agents'))
        await symlink('../elsewhere', path.join(target, '.agents/skills'))
      })

      it('should refuse without --force and leave the link', async () => {
        await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/links elsewhere/)
        expect(await readlink(path.join(target, '.agents/skills'))).toBe('../elsewhere')
      })
    })

    describe('and the install is forced', () => {
      beforeEach(async () => {
        await mkdir(path.join(target, 'elsewhere'))
        await writeFile(path.join(target, 'elsewhere/keep.md'), 'kept')
        await mkdir(path.join(target, '.agents'))
        await symlink('../elsewhere', path.join(target, '.agents/skills'))
      })

      it('should replace only the link with --force', async () => {
        const result = await installKit({ target, linkAgents: true, force: true })

        expect(result.agentsLink).toBe('replaced')
        expect(await readlink(path.join(target, '.agents/skills'))).toBe('../.claude/skills')
        expect(await readFile(path.join(target, 'elsewhere/keep.md'), 'utf8')).toBe('kept')
      })
    })
  })
})

describe('checkKit', () => {
  let target: string
  beforeEach(async () => {
    target = await temp('consumer-')
  })

  describe('when the installed skill matches the library', () => {
    beforeEach(async () => {
      await installKit({ target })
    })

    it('should pass', async () => {
      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
  })

  describe('when the skill was never installed', () => {
    it('should report not-installed', async () => {
      const result = await checkKit({ target })

      expect(result.problems.map((p) => p.reason)).toEqual(['not-installed', 'not-installed'])
    })
  })

  describe('when the installed skill differs only in build metadata', () => {
    beforeEach(async () => {
      await installKit({ target, kitRoot: await kitAtVersion(`${packageVersion}+build.1`) })
    })

    it('should report it as stale and name plain install, which accepts it', async () => {
      const messages = (await checkKit({ target })).problems.map((p) => p.message)

      expect(messages).toHaveLength(2)
      for (const message of messages) {
        expect(message).toMatch(/build metadata\. Run install\.$/)
        expect(message).not.toMatch(/--force/)
      }
      expect((await installKit({ target })).installed).toHaveLength(2)
    })
  })

  describe('when the installed skill is newer than the library', () => {
    beforeEach(async () => {
      await installKit({ target, kitRoot: await kitAtVersion(newerVersion) })
    })

    it('should report it as stale and name install --force, which install requires', async () => {
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
    beforeEach(async () => {
      await installKit({ target })
      await writeFile(path.join(target, '.claude/skills/implement-figma-design/.kit.json'), 'oops')
    })

    it('should report it as modified instead of crashing', async () => {
      const result = await checkKit({ target })

      expect(result.problems).toEqual([
        expect.objectContaining({ skill: 'implement-figma-design', reason: 'modified' })
      ])
    })
  })

  describe('when a skill path is a file', () => {
    beforeEach(async () => {
      await installKit({ target })
      const skill = path.join(target, '.claude/skills/implement-figma-design')
      await rm(skill, { recursive: true })
      await writeFile(skill, 'not a directory')
    })

    it('should report it as not installed instead of crashing', async () => {
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
    beforeEach(async () => {
      const outside = await temp('outside-')
      await installKit({ target: outside })
      await symlink(path.join(outside, '.claude'), path.join(target, '.claude'))
    })

    it('should fail without following it', async () => {
      expect(await checkKit({ target })).toMatchObject({
        ok: false,
        problems: [{ skill: '.claude/skills', reason: 'not-installed' }]
      })
    })
  })

  describe('when an installed file has CRLF line endings', () => {
    let skill: string
    beforeEach(async () => {
      await installKit({ target })
      skill = path.join(target, '.claude/skills/implement-figma-design/SKILL.md')
    })

    describe('and only the line endings changed', () => {
      beforeEach(async () => {
        await writeFile(skill, (await readFile(skill, 'utf8')).replaceAll('\n', '\r\n'))
      })

      it('should pass, as a Windows checkout with core.autocrlf would have it', async () => {
        expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
      })
    })

    describe('and the text itself changed too', () => {
      beforeEach(async () => {
        await writeFile(skill, `${(await readFile(skill, 'utf8')).replaceAll('\n', '\r\n')}x`)
      })

      it('should still report the change to the text', async () => {
        expect((await checkKit({ target })).problems.map((p) => p.reason)).toEqual(['modified'])
      })
    })
  })

  describe('when the install was made by a kit that hashed files the earlier way', () => {
    const legacyHash = async (dir: string) => {
      const hash = createHash('sha256')
      for (const file of (await readdir(dir)).filter((name) => name !== '.kit.json').sort()) {
        hash.update(file)
        hash.update(await readFile(path.join(dir, file)))
      }
      return hash.digest('hex')
    }
    const relabel = async (dir: string) => {
      const manifest = JSON.parse(await readFile(path.join(dir, '.kit.json'), 'utf8'))
      await writeFile(
        path.join(dir, '.kit.json'),
        JSON.stringify({ ...manifest, hash: await legacyHash(dir) })
      )
    }

    beforeEach(async () => {
      await installKit({ target })
    })

    describe('and the files were not edited since', () => {
      beforeEach(async () => {
        for (const name of ['implement-figma-design', 'maintain-figma-library'])
          await relabel(path.join(target, '.claude/skills', name))
      })

      it('should report it as stale, not as edited, and let install repair it', async () => {
        expect((await checkKit({ target })).problems.map((p) => p.reason)).toEqual([
          'stale',
          'stale'
        ])
        expect((await installKit({ target })).installed).toHaveLength(2)
        expect(await checkKit({ target })).toMatchObject({ ok: true })
      })
    })

    describe('and a file was edited after such an install', () => {
      beforeEach(async () => {
        const dir = path.join(target, '.claude/skills/implement-figma-design')
        await relabel(dir)
        await writeFile(path.join(dir, 'SKILL.md'), 'edited')
      })

      it('should still report the edit as modified', async () => {
        expect((await checkKit({ target })).problems.map((p) => p.reason)).toEqual(['modified'])
      })
    })
  })

  describe('when a file is renamed and its content shifted into the name', () => {
    beforeEach(async () => {
      await installKit({ target })
      const dir = path.join(target, '.claude/skills/implement-figma-design')
      const content = await readFile(path.join(dir, 'SKILL.md'))
      await rm(path.join(dir, 'SKILL.md'))
      await writeFile(
        path.join(dir, `SKILL.md${content.subarray(0, 1).toString()}`),
        content.subarray(1)
      )
    })

    it('should report it as modified', async () => {
      expect((await checkKit({ target })).problems.map((p) => p.reason)).toEqual(['modified'])
    })
  })

  describe('when the library was upgraded after install', () => {
    let kitRoot: string
    beforeEach(async () => {
      await installKit({ target })
      kitRoot = await kitAtVersion(newerVersion)
    })

    it('should report the skill as stale', async () => {
      const result = await checkKit({ target, kitRoot })

      expect(result.problems.map((p) => p.reason)).toEqual(['stale', 'stale'])
    })
  })

  describe('when the installed skill was edited by hand', () => {
    beforeEach(async () => {
      await installKit({ target })
      await writeFile(path.join(target, '.claude/skills/implement-figma-design/SKILL.md'), 'edited')
    })

    it('should report it as modified', async () => {
      const result = await checkKit({ target })

      expect(result.problems.map((p) => p.reason)).toEqual(['modified'])
    })
  })

  describe('when the kit source changes without a reinstall', () => {
    let kitRoot: string
    beforeEach(async () => {
      kitRoot = await kitAtVersion(packageVersion)
      await installKit({ target, kitRoot })
      await writeFile(path.join(kitRoot, 'skills/implement-figma-design/SKILL.md'), 'changed')
    })

    it('should report the installed skill as stale', async () => {
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
    beforeEach(async () => {
      await installKit({ target, linkAgents: true })
      await rm(path.join(target, '.agents/skills'))
      await make(path.join(target, '.agents/skills'))
    })

    it('should report agents-link-mismatch after an install that asked for the link', async () => {
      const result = await checkKit({ target })

      expect(result).toMatchObject({ ok: false })
      expect(result.problems.map((p) => p.reason)).toEqual(['agents-link-mismatch'])
    })
  })

  describe('when the consumer owns .agents/skills and installed without the link', () => {
    describe.each([
      ['a real directory', (link: string) => mkdir(link, { recursive: true })],
      [
        'a link elsewhere',
        async (link: string) => {
          await mkdir(path.dirname(link), { recursive: true })
          await symlink('../elsewhere', link)
        }
      ]
    ])('and .agents/skills is %s', (_label, make) => {
      beforeEach(async () => {
        await make(path.join(target, '.agents/skills'))
        await installKit({ target })
      })

      it('should pass', async () => {
        expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
      })
    })
  })

  describe('when the kit link is spelled differently but resolves to the skills', () => {
    describe.each(['./../.claude/skills', '../.claude/skills/'])('and it is %s', (spelling) => {
      beforeEach(async () => {
        await installKit({ target, linkAgents: true })
        await rm(path.join(target, '.agents/skills'))
        await symlink(spelling, path.join(target, '.agents/skills'))
      })

      it('should accept the spelling', async () => {
        expect((await installKit({ target, linkAgents: true })).agentsLink).toBe('already-linked')
        expect(await checkKit({ target })).toMatchObject({ ok: true })
      })
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
    beforeEach(async () => {
      await make(target)
    })

    it('should refuse the link instead of writing outside the target', async () => {
      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/is not a link to/)
      expect(
        await lstat(path.join(target, 'outside/skills')).catch(() => undefined)
      ).toBeUndefined()
    })
  })

  describe('when .agents/skills is a broken symlink', () => {
    beforeEach(async () => {
      await mkdir(path.join(target, '.agents'))
      await symlink('../missing', path.join(target, '.agents/skills'))
    })

    it('should refuse it without --force and replace it with --force', async () => {
      await expect(installKit({ target, linkAgents: true })).rejects.toThrow(/links elsewhere/)
      expect(await lstat(path.join(target, '.claude')).catch(() => undefined)).toBeUndefined()
      expect((await installKit({ target, linkAgents: true, force: true })).agentsLink).toBe(
        'replaced'
      )
    })
  })

  describe('when .agents/skills is the kit link', () => {
    beforeEach(async () => {
      await installKit({ target, linkAgents: true })
    })

    it('should pass', async () => {
      expect(await checkKit({ target })).toMatchObject({ ok: true, problems: [] })
    })
  })
})

describe('the figma-agent-kit command', () => {
  let target: string
  const cli = (args: string[]) =>
    spawnSync(process.execPath, [path.join(import.meta.dirname, 'cli.mjs'), ...args], {
      encoding: 'utf8'
    })
  beforeEach(async () => {
    target = await temp('consumer-')
  })

  describe('when --no-agents-link follows --agents-link', () => {
    it('should let --no-agents-link override --agents-link', async () => {
      const run = cli(['install', '--target', target, '--agents-link', '--no-agents-link'])

      expect(run.status).toBe(0)
      expect(JSON.parse(run.stdout)).toMatchObject({ agentsLink: 'skipped' })
      expect(await lstat(path.join(target, '.agents')).catch(() => undefined)).toBeUndefined()
    })
  })

  describe('when running check before and after an install', () => {
    it('should exit 0 on a passing check and 1 on a failing one', async () => {
      expect(cli(['check', '--target', target]).status).toBe(1)
      expect(cli(['install', '--target', target]).status).toBe(0)
      expect(cli(['check', '--target', target]).status).toBe(0)
    })
  })

  describe('when install refuses', () => {
    beforeEach(async () => {
      await mkdir(path.join(target, '.claude/skills/implement-figma-design'), { recursive: true })
    })

    it('should exit 1 with the reason on stderr', async () => {
      const run = cli(['install', '--target', target])

      expect([run.status, run.stdout]).toEqual([1, ''])
      expect(run.stderr).toMatch(/not installed by this kit/)
    })
  })

  describe('when the arguments name no known command', () => {
    it.each([[[]], [['unknown']]])('should exit 2 with the usage for %j', (args) => {
      const run = cli(args)

      expect(run.status).toBe(2)
      expect(run.stderr).toMatch(/^Usage:/)
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
