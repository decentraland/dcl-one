import { describe, expect, it } from 'vitest'
import { publicEntry, relocate, supersededDirectories, supersession } from './promote.js'

describe('relocate', () => {
  describe('when a reviewed preview imports the Review runtime, matrix type and a sibling', () => {
    it('should point them at the library runtime, a local type and the sibling directory', () => {
      const source = [
        "import { FigmaView } from '../../../components/figma-module-runtime'",
        "import type { MatrixCase } from '../../../components/StateMatrix'",
        "import '../nav-bar/NavMenuTab'"
      ].join('\n')

      const relocated = relocate(source)

      expect(relocated).toContain("from '../runtime/figma-module-runtime'")
      expect(relocated).toContain('type MatrixCase = {')
      expect(relocated).toContain("import '../NavMenuTab/NavMenuTab'")
    })
  })

  describe('when a reviewed CSS Module composes a text style', () => {
    it('should compose it from the typography foundation', () => {
      const relocated = relocate(
        ".a { composes: text6865 from '../../tokens/text-styles.module.css'; }"
      )

      expect(relocated).toBe(
        ".a { composes: text6865 from '../../foundations/typography/text-styles.module.css'; }"
      )
    })
  })
})

const handWritten = [
  "export { Button } from './components/Button/Button'",
  "export type { ButtonProps, ButtonVariant } from './components/Button/Button'",
  "export { Modal } from './components/Modal/Modal'",
  "export type { ModalProps } from './components/Modal/Modal'"
].join('\n')
const promotedButton = [
  "export { Button } from './components/Button/Button'",
  "export type { ButtonProps } from './components/Button/Button'"
].join('\n')

describe('supersededDirectories', () => {
  describe('when a promoted name is exported by a hand-written directory', () => {
    it('should return that directory', () => {
      expect(supersededDirectories(handWritten, ['Button', 'NavBar'])).toEqual(['Button'])
    })
  })

  describe('when no promoted name matches a hand-written export', () => {
    it('should keep every hand-written component', () => {
      expect(supersededDirectories(handWritten, ['NavBar'])).toEqual([])
    })
  })

  describe('when the name is exported from promotion output', () => {
    it('should never return the runtime, the assets or the promoted block', () => {
      const entry = [
        "export { FigmaView } from './components/runtime/figma-module-runtime'",
        '// BEGIN PROMOTED COMPONENTS',
        "export { NavBar } from './components/NavBar/NavBar'",
        '// END PROMOTED COMPONENTS'
      ].join('\n')

      expect(supersededDirectories(entry, ['FigmaView', 'NavBar'])).toEqual([])
    })
  })
})

describe('supersession', () => {
  describe('when promotion.include names the synced component', () => {
    it('should supersede the hand-written directory', () => {
      expect(supersession(handWritten, ['Button', 'NavBar'], ['NavBar', 'Button'])).toEqual([
        'Button'
      ])
    })
  })

  describe('when the synced component is selected only as a preview dependency', () => {
    it('should supersede nothing', () => {
      expect(supersession(handWritten, ['Button', 'NavBar'], ['NavBar'])).toEqual([])
    })
  })

  describe('when promotion.include is absent and every eligible component is selected', () => {
    it('should supersede nothing', () => {
      expect(supersession(handWritten, ['Button', 'NavBar'], undefined)).toEqual([])
    })
  })

  describe('when the replacement was promoted on an earlier run', () => {
    it('should supersede nothing, since only the promoted block exports it', () => {
      const entry = publicEntry(handWritten, ['Button'], [promotedButton])

      expect(supersession(entry, ['Button'], ['Button'])).toEqual([])
    })
  })
})

describe('publicEntry', () => {
  describe('when a hand-written directory is superseded', () => {
    it('should drop its exports and export the promoted component once', () => {
      const entry = publicEntry(handWritten, ['Button'], [promotedButton])

      expect(entry).not.toContain('ButtonVariant')
      expect(entry).toContain("export { Modal } from './components/Modal/Modal'")
      expect(entry.match(/export \{ Button \}/g)).toHaveLength(1)
    })
  })

  describe('when promotion runs a second time over its own output', () => {
    it('should produce the same entry', () => {
      const first = publicEntry(handWritten, ['Button'], [promotedButton])
      const second = publicEntry(first, supersession(first, ['Button'], ['Button']), [
        promotedButton
      ])

      expect(second).toBe(first)
    })
  })

  describe('when nothing is superseded', () => {
    it('should keep every hand-written export', () => {
      expect(publicEntry(handWritten, [], [])).toContain(handWritten)
    })
  })
})
