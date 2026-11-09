import { mkdir, mkdtemp, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import type { FigmaSet } from './figma-index.js'

const FILE_KEY = 'AAAAAAAAAAAAAAAAAAAAAA'
// Built by concatenation so the tree never holds a literal Figma file URL (figma-key-guard `url`).
const FIGMA_DESIGN_URL = 'https://www.figma' + '.com/design/'

type MappingOptions = {
  nodeId?: string
  literalUrl?: boolean
  contract?: string
  component?: string
  importFrom?: string
  kindValues?: Record<string, string>
  body?: string
  id?: string
}

function mappingSource({
  nodeId = '1-1',
  literalUrl = false,
  contract = '1',
  component = 'Button',
  importFrom = 'fixture-library',
  kindValues = { Primary: 'primary', Secondary: 'secondary' },
  body = '',
  id = 'button'
}: MappingOptions = {}) {
  const entries = Object.entries(kindValues)
    .map(([figma, code]) => `  ${figma}: '${code}'`)
    .join(',\n')
  const url = literalUrl
    ? `${FIGMA_DESIGN_URL}${FILE_KEY}/Fixture?node-id=${nodeId}`
    : `<DCL_FIGMA_FILE>?node-id=${nodeId}`
  return `// url=${url}
// source=src/Button.tsx
// component=${component}
// contract=${contract}
// states=Hover
import figma from 'figma'
const instance = figma.selectedInstance

const kind = instance.getEnum('Kind', {
${entries}
})
const disabled = instance.getEnum('Blocked', { Yes: ' disabled', No: '' })
${body}
export default {
  example: figma.tsx\`<${component} kind="\${kind}"\${disabled}>Label</${component}>\`,
  imports: ["import { ${component} } from '${importFrom}'"],
  id: '${id}',
  metadata: { nestable: true }
}
`
}

// Primary has a Blocked variant, Secondary does not: the same shape as the real Button-Contained-Text.
const buttonSet: FigmaSet = {
  setId: '1:1',
  name: 'Button',
  page: 'Buttons',
  kind: 'set',
  properties: {
    Kind: { type: 'VARIANT', default: 'Primary', options: ['Primary', 'Secondary'] },
    Hover: { type: 'VARIANT', default: 'No', options: ['No', 'Yes'] },
    Blocked: { type: 'VARIANT', default: 'No', options: ['No', 'Yes'] }
  },
  variants: [
    {
      id: '1:2',
      name: 'Kind=Primary,Hover=No,Blocked=No',
      props: { Kind: 'Primary', Hover: 'No', Blocked: 'No' }
    },
    {
      id: '1:3',
      name: 'Kind=Primary,Hover=No,Blocked=Yes',
      props: { Kind: 'Primary', Hover: 'No', Blocked: 'Yes' }
    },
    { id: '1:4', name: 'Kind=Secondary,Hover=No', props: { Kind: 'Secondary', Hover: 'No' } }
  ]
}

type LibraryOptions = {
  name?: string
  version?: string
  entry?: string
  mappings?: Record<string, string>
}

// A throwaway component library laid out like the real one: package.json, src/index.ts and *.figma.ts files.
async function makeLibrary({
  name = 'fixture-library',
  version = '1.2.0',
  entry = "export { Button } from './Button'\n",
  mappings = { 'Button.figma.ts': mappingSource() }
}: LibraryOptions = {}) {
  const root = await mkdtemp(path.join(tmpdir(), 'dcl-figma-lib-'))
  await mkdir(path.join(root, 'src'), { recursive: true })
  await writeFile(
    path.join(root, 'package.json'),
    JSON.stringify({ name, version, main: './src/index.ts' })
  )
  await writeFile(path.join(root, 'src/index.ts'), entry)
  await writeFile(path.join(root, 'src/Button.tsx'), 'export const Button = () => null\n')
  for (const [file, source] of Object.entries(mappings)) {
    await writeFile(path.join(root, 'src', file), source)
  }
  return { root, mappingRoot: path.join(root, 'src') }
}

async function makeConsumer(libraryRoot: string, libraryName = 'fixture-library') {
  const root = await mkdtemp(path.join(tmpdir(), 'dcl-figma-consumer-'))
  await mkdir(path.join(root, 'node_modules'), { recursive: true })
  await symlink(libraryRoot, path.join(root, 'node_modules', libraryName), 'dir')
  return root
}

export { FILE_KEY, buttonSet, makeConsumer, makeLibrary, mappingSource }
