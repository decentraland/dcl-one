// Copies the saved capture into test/fixtures so unit tests do not change when Figma does.
// Run it only when a test intentionally needs newer capture data, then run key-guard over the result.
import { cp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'

const root = path.resolve(import.meta.dirname, '..')
const fixtures = path.join(root, 'test/fixtures')
const source = path.join(root, 'figma-snapshot')
const target = path.join(fixtures, 'figma-snapshot')
const library = path.resolve(root, '..')

// The capture is stored in Git LFS: copying a pointer would freeze a pointer, not the capture.
async function materialized(file) {
  const content = await readFile(file)
  if (content.subarray(0, 40).toString('utf8').startsWith('version https://git-lfs'))
    throw new Error(`${path.relative(root, file)} is a Git LFS pointer; run git lfs pull first`)
  return content
}

// Contract tests resolve against these mappings, not the live library that promotion rewrites.
await rm(path.join(fixtures, 'library'), { recursive: true, force: true })
for (const file of ['package.json', 'components/Button', 'components/Modal'])
  await cp(path.join(library, file), path.join(fixtures, 'library', file), {
    recursive: true,
    filter: (file) => !file.endsWith('.test.tsx')
  })

// The fixture config never names the file: the key comes from FIGMA_FILE_KEY only.
const config = JSON.parse(await readFile(path.join(root, 'figma.config.json'), 'utf8'))
delete config.fileKey
await writeFile(
  path.join(fixtures, 'figma.config.json'),
  JSON.stringify(
    {
      ...config,
      snapshotDir: 'figma-snapshot',
      outDir: 'generated',
      contract: { ...config.contract, mappingRoot: 'library/components' }
    },
    null,
    2
  ) + '\n'
)

// Components above this serialized size are raster-heavy and no test reads them.
const MAX_COMPONENT_BYTES = 300_000

async function copyTree(from, to) {
  await mkdir(to, { recursive: true })
  for (const item of await readdir(from, { withFileTypes: true })) {
    const file = path.join(from, item.name)
    if (item.isDirectory()) await copyTree(file, path.join(to, item.name))
    else await writeFile(path.join(to, item.name), await materialized(file))
  }
}

await rm(target, { recursive: true, force: true })
await mkdir(path.join(target, 'components'), { recursive: true })
for (const item of await readdir(source, { withFileTypes: true })) {
  if (item.name === 'components') continue
  const from = path.join(source, item.name)
  if (item.isDirectory()) await copyTree(from, path.join(target, item.name))
  else await writeFile(path.join(target, item.name), await materialized(from))
}
for (const file of await readdir(path.join(source, 'components'))) {
  const page = JSON.parse(await materialized(path.join(source, 'components', file)))
  page.components = page.components.filter(
    (component) => JSON.stringify(component).length <= MAX_COMPONENT_BYTES
  )
  await writeFile(path.join(target, 'components', file), JSON.stringify(page))
}
