import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { emptyRetirements } from './retirements.js'

export const GEN = 'figma/review/src/generated'
export const REVIEW = 'figma/review/src/review'
export const SNAP = 'figma/figma-snapshot'
export const BUTTONS = '1:1'
export const PLACES = '2:2'

export const components = [
  { name: 'Button', page: 'Buttons', id: 'b1', file: 'buttons/Button', variants: [{ id: '10:1' }] },
  {
    name: 'PlaceCard',
    page: 'Places',
    id: 'p1',
    file: 'places/PlaceCard',
    variants: [{ id: '20:1' }]
  }
]

export async function put(root: string, file: string, content: string | object) {
  const target = path.join(root, file)
  await mkdir(path.dirname(target), { recursive: true })
  await writeFile(target, typeof content === 'string' ? content : JSON.stringify(content, null, 2))
}

export const manifest = (list = components) =>
  `export const componentManifest: ComponentManifestEntry[] = ${JSON.stringify(list, null, 2)}\n`
export const index = (list = components) =>
  `// header\n${list.map((c) => `export * from './${c.file}'`).join('\n')}\n`

export async function writeTree(root: string) {
  await put(root, `${SNAP}/sync-state.json`, {
    schema: 1,
    pages: [
      { id: BUTTONS, name: 'Buttons', ready: true },
      { id: PLACES, name: 'Places', ready: false }
    ]
  })
  await put(root, `${SNAP}/components/Buttons.json`, {
    v: 1,
    page: { id: BUTTONS, name: 'Buttons' },
    components: [{ id: 'b1', name: 'Button', base: { id: '10:1' }, others: [] }]
  })
  await put(root, `${SNAP}/components/Places.json`, {
    v: 1,
    page: { id: PLACES, name: 'Places' },
    components: [{ id: 'p1', name: 'PlaceCard', base: { id: '20:1' }, others: [] }]
  })
  await put(root, `${GEN}/components/manifest.ts`, manifest())
  await put(root, `${GEN}/components/index.ts`, index())
  await put(root, `${GEN}/components/buttons/Button.tsx`, 'button v1')
  await put(root, `${GEN}/components/buttons/Button.module.css`, 'css v1')
  await put(root, `${GEN}/components/places/PlaceCard.tsx`, 'place v1')
  await put(root, `${GEN}/tokens/tokens.css`, 'tokens v1')
  await put(root, `${REVIEW}/catalog.json`, {
    components: components.map(({ name, page, id, variants }) => ({ name, page, id, variants })),
    pages: [
      { id: BUTTONS, name: 'Buttons' },
      { id: PLACES, name: 'Places' }
    ],
    foundations: { tokens: [] }
  })
  // The promoted library: generated copies, public exports (value and Props type) and a hand-written Modal.
  await put(root, 'components/buttons/Button.tsx', 'lib button')
  await put(root, 'components/places/PlaceCard.tsx', 'lib place')
  await put(root, 'components/places/PlaceCard.figma.ts', 'mapping')
  await put(root, 'components/Modal/Modal.tsx', 'hand-written modal')
  await put(
    root,
    'index.ts',
    [
      "export { Modal } from './Modal/Modal'",
      '// BEGIN PROMOTED COMPONENTS',
      "export { Button } from './components/buttons/Button'",
      "export type { ButtonProps } from './components/buttons/Button'",
      "export { PlaceCard } from './components/places/PlaceCard'",
      "export type { PlaceCardProps } from './components/places/PlaceCard'",
      '// END PROMOTED COMPONENTS',
      ''
    ].join('\n')
  )
  await put(root, 'components/promotion.json', {
    schema: 1,
    files: {},
    components: ['Button', 'PlaceCard']
  })
  await put(root, `${SNAP}/retirements.json`, emptyRetirements())
}
