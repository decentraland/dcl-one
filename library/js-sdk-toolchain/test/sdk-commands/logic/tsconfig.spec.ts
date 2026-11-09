import fs from 'fs'
import os from 'os'
import path from 'path'
import { createFsComponent } from '../../../packages/@dcl/sdk-commands/src/components/fs'
import { fixTsconfigForTypescript7 } from '../../../packages/@dcl/sdk-commands/src/logic/tsconfig'
import { typeCheckerBin } from '../../../packages/@dcl/sdk-commands/src/logic/bundle'
import { initLanguage, Language } from '../../../packages/@dcl/sdk-commands/src/logic/lang'

// the package the scene template's `@dcl/asset-packs` paths are pointed at, as the fixer finds it
const smartItems = path.dirname(
  require.resolve('@dcl/smart-items/package.json', {
    paths: [path.resolve(__dirname, '../../../packages/@dcl/sdk-commands/src/logic')]
  })
)

function smartItemsFrom(dir: string) {
  const relative = path.relative(dir, smartItems).split(path.sep).join('/')
  return relative.startsWith('../') ? relative : `./${relative}`
}

const TEMPLATE = `{
  // the scene template's config
  "compilerOptions": {
    "allowJs": true,
    "strict": true,
    "baseUrl": ".",
    "paths": {
      "@dcl/asset-packs/*": ["node_modules/@dcl/inspector/node_modules/@dcl/asset-packs/*"]
    }
  },
  "include": ["src/**/*.ts"],
  "extends": "@dcl/sdk/types/tsconfig.ecs7.json"
}
`

describe('fixTsconfigForTypescript7', () => {
  let dir: string
  let logger: { log: jest.Mock }
  let components: Parameters<typeof fixTsconfigForTypescript7>[0]

  beforeAll(async () => {
    await initLanguage(Language.EN)
  })

  beforeEach(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tsconfig-fix-'))
    logger = { log: jest.fn() }
    components = { logger: logger as any, fs: createFsComponent() }
  })

  afterEach(() => {
    fs.rmSync(dir, { recursive: true, force: true })
  })

  function write(name: string, text: string) {
    const file = path.join(dir, name)
    fs.mkdirSync(path.dirname(file), { recursive: true })
    fs.writeFileSync(file, text)
    return file
  }

  function read(name: string) {
    return fs.readFileSync(path.join(dir, name), 'utf-8')
  }

  async function fix(name = 'tsconfig.json') {
    return fixTsconfigForTypescript7(components, path.join(dir, name), dir)
  }

  it('rewrites the scene template: baseUrl dropped, asset-packs paths pointed at @dcl/smart-items, "*" added, comments kept', async () => {
    const file = write('tsconfig.json', TEMPLATE)

    expect(await fix()).toEqual([file])

    expect(read('tsconfig.json')).toBe(`{
  // the scene template's config
  "compilerOptions": {
    "allowJs": true,
    "strict": true,
    "paths": {
      "@dcl/asset-packs/*": ["${smartItemsFrom(dir)}/*"],
      "*": ["./*"],
      "@dcl/asset-packs": ["${smartItemsFrom(dir)}/dist/index"]
    }
  },
  "include": ["src/**/*.ts"],
  "extends": "@dcl/sdk/types/tsconfig.ecs7.json"
}
`)
    expect(logger.log).toHaveBeenCalledTimes(3)
    expect(logger.log.mock.calls[0][0]).toContain(file)
    expect(logger.log.mock.calls[1][0]).toContain('./*')
    expect(logger.log.mock.calls[2][0]).toContain(smartItemsFrom(dir))

    // a second run finds nothing to do
    logger.log.mockClear()
    expect(await fix()).toEqual([])
    expect(logger.log).not.toHaveBeenCalled()
  })

  it('resolves paths through a baseUrl below the file and keeps absolute and non-string entries', async () => {
    write(
      'tsconfig.json',
      `{
  "compilerOptions": {
    "baseUrl": "src",
    "paths": {
      "lib/*": ["lib/*", "../vendor/*", "/abs/*", 7],
      "up": [".."],
      "same": ["."]
    }
  }
}
`
    )

    await fix()

    expect(JSON.parse(read('tsconfig.json')).compilerOptions).toEqual({
      paths: {
        'lib/*': ['./src/lib/*', './vendor/*', '/abs/*', 7],
        up: ['.'],
        same: ['./src'],
        '*': ['./src/*']
      },
      strict: false
    })
  })

  it('keeps an existing "*" entry and re-roots it', async () => {
    write('tsconfig.json', `{ "compilerOptions": { "baseUrl": ".", "paths": { "*": ["*", "./src/*"] } } }\n`)

    await fix()

    expect(read('tsconfig.json')).toBe(
      `{ "compilerOptions": { "paths": { "*": ["./*", "./src/*"] }, "strict": false } }\n`
    )
    expect(logger.log).toHaveBeenCalledTimes(2)
  })

  it('adds paths when the file has none, and fills an empty one', async () => {
    write('tsconfig.json', `{\n  "compilerOptions": {\n    "strict": true,\n    "baseUrl": "./src"\n  }\n}\n`)
    await fix()
    expect(read('tsconfig.json')).toBe(
      `{\n  "compilerOptions": {\n    "strict": true,\n    "paths": { "*": ["./src/*"] }\n  }\n}\n`
    )

    write('empty.json', `{ "compilerOptions": { "paths": {}, "baseUrl": "." } }\n`)
    await fix('empty.json')
    expect(read('empty.json')).toBe(`{ "compilerOptions": { "paths": { "*": ["./*"] }, "strict": false } }\n`)

    write('sole.json', `{ "compilerOptions": { "baseUrl": "." }, "include": [] }\n`)
    await fix('sole.json')
    expect(read('sole.json')).toBe(
      `{ "compilerOptions": { "paths": { "*": ["./*"] }, "strict": false }, "include": [] }\n`
    )
  })

  it('skips paths that are not an object of arrays', async () => {
    write('tsconfig.json', `{ "compilerOptions": { "baseUrl": ".", "paths": { "a": "src/a" } } }\n`)
    await fix()
    expect(JSON.parse(read('tsconfig.json')).compilerOptions).toEqual({
      paths: { a: 'src/a', '*': ['./*'] },
      strict: false
    })

    write('other.json', `{ "compilerOptions": { "baseUrl": ".", "paths": [] } }\n`)
    await fix('other.json')
    expect(JSON.parse(read('other.json')).compilerOptions).toEqual({ paths: [], strict: false })
  })

  it('points the asset-packs paths at the copy under the scene\'s own node_modules when it has one', async () => {
    write('node_modules/@dcl/smart-items/package.json', '{ "name": "@dcl/smart-items" }')
    write('tsconfig.json', TEMPLATE)

    await fix()

    expect(read('tsconfig.json')).toContain('"@dcl/asset-packs/*": ["./node_modules/@dcl/smart-items/*"]')
    expect(read('tsconfig.json')).toContain('"@dcl/asset-packs": ["./node_modules/@dcl/smart-items/dist/index"]')
    expect(logger.log.mock.calls[2][0]).toContain('./node_modules/@dcl/smart-items')
    expect(await fix()).toEqual([])
  })

  it('uses the copy of an enclosing workspace when the scene has none of its own', async () => {
    write('node_modules/@dcl/smart-items/package.json', '{ "name": "@dcl/smart-items" }')
    const scene = path.join(dir, 'scenes', 'one')
    const file = write('scenes/one/tsconfig.json', TEMPLATE)

    expect(await fixTsconfigForTypescript7(components, file, scene)).toEqual([file])

    expect(read('scenes/one/tsconfig.json')).toContain('"@dcl/asset-packs/*": ["../../node_modules/@dcl/smart-items/*"]')
  })

  it('points asset-packs paths of a file without baseUrl at @dcl/smart-items and keeps the rest', async () => {
    write(
      'tsconfig.json',
      `{ "compilerOptions": { "strict": true, "paths": { "@dcl/asset-packs": ["./x/dist"], "@dcl/asset-packs/dist/*": ["./x/dist/*"], "lib/*": ["./lib/*"] } } }\n`
    )

    await fix()

    expect(JSON.parse(read('tsconfig.json')).compilerOptions.paths).toEqual({
      '@dcl/asset-packs': [`${smartItemsFrom(dir)}/dist/index`],
      '@dcl/asset-packs/dist/*': [`${smartItemsFrom(dir)}/dist/*`],
      'lib/*': ['./lib/*']
    })
    expect(logger.log).toHaveBeenCalledTimes(1)
    expect(await fix()).toEqual([])
  })

  it('leaves a file without baseUrl alone', async () => {
    const text = `{ "compilerOptions": { "strict": true, "paths": { "lib/*": ["lib/*"] } } }\n`
    write('tsconfig.json', text)

    expect(await fix()).toEqual([])
    expect(read('tsconfig.json')).toBe(text)
    expect(logger.log).not.toHaveBeenCalled()
  })

  it('follows relative extends inside the project, with and without the .json extension, and inherits baseUrl', async () => {
    const base = write(
      'config/base',
      `{\n\t"compilerOptions": {\n\t\t"baseUrl": "..",\n\t\t"paths": {\n\t\t\t"shared/*": ["shared/*"]\n\t\t}\n\t}\n}\n`
    )
    const mid = write(
      'config/mid.json',
      `{\r\n  "extends": ["./base"],\r\n  "compilerOptions": { "paths": { "lib/*": ["lib/*"] } }\r\n}\r\n`
    )
    const leaf = write(
      'tsconfig.json',
      `{"extends":"./config/mid.json","compilerOptions":{"paths":{"app/*":["src/app/*"]}}}`
    )

    expect(await fix()).toEqual([leaf, mid, base])

    // leaf: paths resolve against the inherited baseUrl (the project root) and are written relative to the leaf;
    // its paths are the ones in force, so the "*" entry lands here
    expect(read('tsconfig.json')).toBe(
      `{"extends":"./config/mid.json","compilerOptions":{"paths":{"app/*":["./src/app/*"], "*": ["./*"]}, "strict": false}}`
    )
    // mid: inherited baseUrl is the project root, written relative to config/
    expect(read('config/mid.json')).toBe(
      `{\r\n  "extends": ["./base"],\r\n  "compilerOptions": { "paths": { "lib/*": ["../lib/*"], "*": ["../*"] } }\r\n}\r\n`
    )
    // base: its own baseUrl goes, "*" is added with the file's indentation
    expect(read('config/base')).toBe(
      `{\n\t"compilerOptions": {\n\t\t"paths": {\n\t\t\t"shared/*": ["../shared/*"],\n\t\t\t"*": ["../*"]\n\t\t}\n\t}\n}\n`
    )
  })

  it('reads the configs in node_modules and outside the project without rewriting them', async () => {
    const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'tsconfig-outside-'))
    const outsideFile = path.join(outside, 'tsconfig.json')
    fs.writeFileSync(outsideFile, `{ "compilerOptions": { "baseUrl": ".", "strict": true } }\n`)
    write('node_modules/pkg/tsconfig.json', `{ "compilerOptions": { "baseUrl": "." } }\n`)
    const text = JSON.stringify({
      extends: ['./node_modules/pkg/tsconfig.json', outsideFile],
      compilerOptions: { paths: { 'lib/*': ['lib/*'] } }
    })
    write('tsconfig.json', text)

    try {
      // the scene's paths stay bare: an external baseUrl is not inherited; strict comes from the outside file
      expect(await fix()).toEqual([])
      expect(read('tsconfig.json')).toBe(text)
      expect(read('node_modules/pkg/tsconfig.json')).toContain('baseUrl')
      expect(fs.readFileSync(outsideFile, 'utf-8')).toContain('baseUrl')
      expect(logger.log).not.toHaveBeenCalled()
    } finally {
      fs.rmSync(outside, { recursive: true, force: true })
    }
  })

  it('reads a file reached twice once and stops at the chain limit', async () => {
    write('a.json', `{ "compilerOptions": { "baseUrl": "." } }\n`)
    write('tsconfig.json', `{ "extends": ["./a.json", "./a.json"], "compilerOptions": { "paths": { "x": ["x"] } } }\n`)
    expect(await fix()).toEqual([path.join(dir, 'tsconfig.json'), path.join(dir, 'a.json')])

    const chain = (length: number) => {
      for (let i = 0; i < length; i++) {
        write(
          `chain${length}/${i}.json`,
          JSON.stringify({ extends: `./${i + 1}.json`, compilerOptions: { paths: { x: ['x'] } } })
        )
      }
      write(`chain${length}/${length}.json`, `{ "compilerOptions": { "baseUrl": "." } }\n`)
    }
    // the baseUrl of the 6th file reaches the 5 files that extend it
    chain(5)
    expect(await fix('chain5/0.json')).toHaveLength(6)
    // the 21st file is past the limit: nothing inherits its baseUrl, it keeps it, and strict is unknown
    chain(20)
    logger.log.mockClear()
    expect(await fix('chain20/0.json')).toEqual([])
    expect(read('chain20/20.json')).toContain('baseUrl')
    expect(read('chain20/0.json')).not.toContain('strict')
    expect(logger.log).toHaveBeenCalledTimes(1)
    expect(logger.log.mock.calls[0][0]).toContain('"./16.json"')
  })

  it('leaves unreadable, unparseable and non-object files to the type checker', async () => {
    expect(await fix('missing.json')).toEqual([])

    write('tsconfig.json', `{ "compilerOptions": { "baseUrl": ". } `)
    expect(await fix()).toEqual([])
    expect(read('tsconfig.json')).toContain('baseUrl')

    write('array.json', `[1, 2]`)
    expect(await fix('array.json')).toEqual([])
    expect(logger.log).not.toHaveBeenCalled()
  })

  it('keeps the pre-TypeScript 7 strict default for a scene that never set it', async () => {
    const file = write(
      'tsconfig.json',
      `{\n  "compilerOptions": {\n    "target": "ES2020"\n  },\n  "include": ["src/**/*.ts"]\n}\n`
    )

    expect(await fix()).toEqual([file])
    expect(read('tsconfig.json')).toBe(
      `{\n  "compilerOptions": {\n    "target": "ES2020",\n    "strict": false\n  },\n  "include": ["src/**/*.ts"]\n}\n`
    )
    expect(logger.log).toHaveBeenCalledTimes(1)
    expect(logger.log.mock.calls[0][0]).toContain('"strict": false')

    write('base.json', `{ "compilerOptions": { "target": "ES2020" } }\n`)
    write('bare.json', `{ "extends": "./base.json" }\n`)
    await fix('bare.json')
    expect(read('bare.json')).toBe(`{ "extends": "./base.json", "compilerOptions": { "strict": false } }\n`)
    expect(read('base.json')).toBe(`{ "compilerOptions": { "target": "ES2020" } }\n`)
  })

  it('leaves strict alone when the scene or a package config it extends sets it', async () => {
    for (const text of [
      `{ "compilerOptions": { "strict": true } }\n`,
      `{ "compilerOptions": { "strict": false } }\n`
    ]) {
      write('tsconfig.json', text)
      expect(await fix()).toEqual([])
      expect(read('tsconfig.json')).toBe(text)
    }

    write('node_modules/@dcl/sdk/types/tsconfig.ecs7.json', `{ "compilerOptions": { "strict": true } }\n`)
    const extending = `{ "extends": "@dcl/sdk/types/tsconfig.ecs7.json", "compilerOptions": { "target": "ES2020" } }\n`
    write('tsconfig.json', extending)
    expect(await fix()).toEqual([])
    expect(read('tsconfig.json')).toBe(extending)

    write('node_modules/pkg/tsconfig.json', `{ "compilerOptions": { "target": "ES2020" } }\n`)
    write('tsconfig.json', `{ "extends": "pkg/tsconfig.json" }\n`)
    expect(await fix()).toEqual([path.join(dir, 'tsconfig.json')])
    expect(read('tsconfig.json')).toBe(`{ "extends": "pkg/tsconfig.json", "compilerOptions": { "strict": false } }\n`)
    expect(read('node_modules/pkg/tsconfig.json')).toBe(`{ "compilerOptions": { "target": "ES2020" } }\n`)
    expect(logger.log).toHaveBeenCalledTimes(1)
  })

  it('warns and goes on when the rewritten file cannot be written', async () => {
    const file = write('tsconfig.json', TEMPLATE)
    components = {
      logger: logger as any,
      fs: { ...createFsComponent(), writeFile: jest.fn().mockRejectedValue(new Error('EACCES')) }
    }

    expect(await fix()).toEqual([])
    expect(read('tsconfig.json')).toBe(TEMPLATE)
    expect(logger.log).toHaveBeenCalledTimes(1)
    expect(logger.log.mock.calls[0][0]).toContain(file)
  })

  it('resolves a package extends as TypeScript does: tsconfig field, tsconfig.json, .json appended, exports', async () => {
    write('node_modules/@tsconfig/strictest/tsconfig.json', `{ "compilerOptions": { "strict": true } }\n`)
    write('node_modules/named/package.json', `{ "tsconfig": "./configs/base.json" }\n`)
    write('node_modules/named/configs/base.json', `{ "compilerOptions": { "strict": true } }\n`)
    // a package config that extends another package: resolved from the ancestors' node_modules
    write('node_modules/plain/tsconfig.base.json', `{ "extends": "@tsconfig/strictest" }\n`)
    write(
      'node_modules/@acme/configs/package.json',
      `{ "exports": { ".": { "default": "./root.json" }, "./tsconfig/*": ["./missing/*.json", "./dist/*.json"] } }\n`
    )
    write('node_modules/@acme/configs/root.json', `{ "compilerOptions": { "strict": true } }\n`)
    write('node_modules/@acme/configs/dist/strict.json', `{ "compilerOptions": { "strict": true } }\n`)

    for (const target of [
      '@tsconfig/strictest',
      'named',
      'plain/tsconfig.base',
      '@acme/configs',
      '@acme/configs/tsconfig/strict'
    ]) {
      const text = `{ "extends": ${JSON.stringify(target)}, "compilerOptions": { "target": "ES2020" } }\n`
      write('tsconfig.json', text)
      expect(await fix()).toEqual([])
      expect(read('tsconfig.json')).toBe(text)
    }
    expect(logger.log).not.toHaveBeenCalled()

    // a resolved package config that never sets strict keeps the old default
    write('node_modules/loose/tsconfig.json', `{ "compilerOptions": { "target": "ES2020" } }\n`)
    write('tsconfig.json', `{ "extends": "loose" }\n`)
    expect(await fix()).toEqual([path.join(dir, 'tsconfig.json')])
    expect(read('tsconfig.json')).toBe(`{ "extends": "loose", "compilerOptions": { "strict": false } }\n`)
  })

  it('leaves strict alone and warns when an extends target cannot be resolved', async () => {
    write('base.json', `{ "compilerOptions": { "strict": true } }\n`)
    write('loose.json', `{ "compilerOptions": { "target": "ES2020" } }\n`)
    write('node_modules/@acme/configs/package.json', `{ "exports": { "./tsconfig/*": "./dist/*.json" } }\n`)
    write('node_modules/@acme/configs/dist/strict.json', `{ "compilerOptions": { "strict": true } }\n`)
    fs.mkdirSync(path.join(dir, 'config'))
    const unresolved: [unknown, string][] = [
      ['@tsconfig/missing', '@tsconfig/missing'],
      // a directory in the project is not a config
      ['./config', './config'],
      // a file the exports map does not expose
      ['@acme/configs/dist/strict.json', '@acme/configs/dist/strict.json'],
      // the later target wins, and it is unknown, even over an earlier strict
      [['./loose.json', './missing.json'], './missing.json'],
      [['./base.json', './missing.json'], './missing.json'],
      [3, '3']
    ]
    for (const [extend, target] of unresolved) {
      const text = `{ "extends": ${JSON.stringify(extend)}, "compilerOptions": { "target": "ES2020" } }\n`
      write('tsconfig.json', text)
      logger.log.mockClear()
      expect(await fix()).toEqual([])
      expect(read('tsconfig.json')).toBe(text)
      expect(logger.log).toHaveBeenCalledTimes(1)
      expect(logger.log.mock.calls[0][0]).toContain(`"${target}"`)
      expect(logger.log.mock.calls[0][0]).toContain('"strict"')
    }

    // an earlier unresolved target does not matter once a later one sets strict
    const text = `{ "extends": ["./missing.json", "./base.json"], "compilerOptions": { "target": "ES2020" } }\n`
    write('tsconfig.json', text)
    logger.log.mockClear()
    expect(await fix()).toEqual([])
    expect(read('tsconfig.json')).toBe(text)
    expect(logger.log).not.toHaveBeenCalled()

    // a circular chain is reported, not followed
    write('a.json', `{ "extends": "./b.json" }\n`)
    write('b.json', `{ "extends": "./a.json" }\n`)
    expect(await fix('a.json')).toEqual([])
    expect(logger.log).toHaveBeenCalledTimes(1)
    expect(logger.log.mock.calls[0][0]).toContain('"./a.json"')
  })

  it('brings the inherited paths along when a file with baseUrl declares none', async () => {
    write('config/base.json', `{ "compilerOptions": { "paths": { "@/*": ["src/*"] } } }\n`)
    write(
      'tsconfig.json',
      `{\n  "extends": "./config/base.json",\n  "compilerOptions": {\n    "strict": true,\n    "baseUrl": "."\n  }\n}\n`
    )

    expect(await fix()).toEqual([path.join(dir, 'tsconfig.json')])
    expect(read('tsconfig.json')).toBe(
      `{\n  "extends": "./config/base.json",\n  "compilerOptions": {\n    "strict": true,\n    "paths": { "@/*": ["./src/*"], "*": ["./*"] }\n  }\n}\n`
    )
    // the parent has no baseUrl in force: its paths, now shadowed, stay
    expect(read('config/base.json')).toBe(`{ "compilerOptions": { "paths": { "@/*": ["src/*"] } } }\n`)
    expect(logger.log).toHaveBeenCalledTimes(2)

    // from a package config too; its "*" entry is kept instead of adding one
    write(
      'node_modules/pkg/tsconfig.json',
      `{ "compilerOptions": { "strict": true, "paths": { "@/*": ["src/*"], "*": ["vendor/*"] } } }\n`
    )
    write('tsconfig.json', `{ "extends": "pkg/tsconfig.json", "compilerOptions": { "baseUrl": "./app" } }\n`)
    logger.log.mockClear()
    await fix()
    expect(read('tsconfig.json')).toBe(
      `{ "extends": "pkg/tsconfig.json", "compilerOptions": { "paths": { "@/*": ["./app/src/*"], "*": ["./app/vendor/*"] } } }\n`
    )
    expect(logger.log).toHaveBeenCalledTimes(1)
  })

  it('keeps the comments around a removed baseUrl', async () => {
    write(
      'tsconfig.json',
      `{
  "compilerOptions": {
    "strict": true, // keep strict
    // "outDir": "dist",
    "baseUrl": "."
  }
}
`
    )
    await fix()
    expect(read('tsconfig.json')).toBe(`{
  "compilerOptions": {
    "strict": true, // keep strict
    // "outDir": "dist",
    "paths": { "*": ["./*"] }
  }
}
`)

    // last property: the comma after the previous value goes, the comments stay where they are, and the
    // inserted strict goes below the comment that trails the last property
    write(
      'tsconfig.json',
      `{
  "compilerOptions": {
    "paths": { "a/*": ["a/*"] }, // app code
    /* bare imports */
    "baseUrl": "." // root
  }
}
`
    )
    await fix()
    expect(read('tsconfig.json')).toBe(`{
  "compilerOptions": {
    "paths": { "a/*": ["./a/*"], "*": ["./*"] }, // app code
    "strict": false
    /* bare imports */
    // root
  }
}
`)

    // a trailing comma after the last property is reused
    write('tsconfig.json', `{\n  "compilerOptions": {\n    "target": "ES2020", // es\n  }\n}\n`)
    await fix()
    expect(read('tsconfig.json')).toBe(
      `{\n  "compilerOptions": {\n    "target": "ES2020", // es\n    "strict": false\n  }\n}\n`
    )

    // inline, first and last
    write('tsconfig.json', `{ "compilerOptions": { "baseUrl": ".", /* a */ "strict": true, /* b */ "paths": {} } }\n`)
    await fix()
    expect(read('tsconfig.json')).toBe(
      `{ "compilerOptions": { /* a */ "strict": true, /* b */ "paths": { "*": ["./*"] } } }\n`
    )
    write('tsconfig.json', `{ "compilerOptions": { "strict": true, "paths": {}, "baseUrl": "." /* root */ } }\n`)
    await fix()
    expect(read('tsconfig.json')).toBe(
      `{ "compilerOptions": { "strict": true, "paths": { "*": ["./*"] } /* root */ } }\n`
    )
  })

  it('names the rewrite that failed when the file cannot be written', async () => {
    components = {
      logger: logger as any,
      fs: { ...createFsComponent(), writeFile: jest.fn().mockRejectedValue(new Error('EACCES')) }
    }

    // strict only
    const file = write('tsconfig.json', `{ "compilerOptions": { "target": "ES2020" } }\n`)
    expect(await fix()).toEqual([])
    expect(logger.log).toHaveBeenCalledTimes(1)
    expect(logger.log.mock.calls[0][0]).toContain(file)
    expect(logger.log.mock.calls[0][0]).toContain('"strict"')
    expect(logger.log.mock.calls[0][0]).not.toContain('baseUrl')

    // both
    write('tsconfig.json', `{ "compilerOptions": { "baseUrl": "." } }\n`)
    logger.log.mockClear()
    expect(await fix()).toEqual([])
    expect(logger.log).toHaveBeenCalledTimes(2)
    expect(logger.log.mock.calls[0][0]).toContain('"baseUrl"')
    expect(logger.log.mock.calls[1][0]).toContain('"strict"')
  })
})

describe('typeCheckerBin', () => {
  it('resolves the installed compiler script', () => {
    const bin = typeCheckerBin()
    expect(bin.endsWith(path.join('lib', 'tsc.js'))).toBe(true)
    expect(fs.existsSync(bin)).toBe(true)
  })
})
