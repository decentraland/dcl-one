import { cp, lstat, mkdir, mkdtemp, readFile, readlink, rm, writeFile } from 'node:fs/promises'
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
    it('should copy the skill, record its version and link .agents/skills', async () => {
      const target = await temp('consumer-')

      const result = await installKit({ target })

      expect(result.installed).toEqual(['implement-figma-design'])
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

  describe('when .agents/skills already exists', () => {
    it('should leave it alone', async () => {
      const target = await temp('consumer-')
      await mkdir(path.join(target, '.agents/skills'), { recursive: true })

      const result = await installKit({ target })

      expect(result.agentsLink).toBe('already-present')
      expect((await lstat(path.join(target, '.agents/skills'))).isSymbolicLink()).toBe(false)
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

      expect(result.problems.map((p) => p.reason)).toEqual(['not-installed'])
    })
  })

  describe('when the library was upgraded after install', () => {
    it('should report the skill as stale', async () => {
      const target = await temp('consumer-')
      await installKit({ target })

      const result = await checkKit({ target, kitRoot: await kitAtVersion(newerVersion) })

      expect(result.problems.map((p) => p.reason)).toEqual(['stale'])
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
})
