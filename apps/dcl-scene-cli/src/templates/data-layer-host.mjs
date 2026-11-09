import { createRequire } from 'node:module'
import path from 'node:path'
import fs from 'node:fs'

const projectDir = path.resolve(process.argv[2])
const mode = process.argv[3] || 'serve'
process.chdir(projectDir)

const inspectorDir = process.env.DCL_ONE_INSPECTOR_DIR || ''
const sceneRequire = createRequire(path.join(projectDir, 'package.json'))
const inspectorRequire = inspectorDir
  ? createRequire(path.join(inspectorDir, 'package.json'))
  : null
// The data-layer host and the composite readers, with the WebSocket rpc server
// and the ecs they run on (`serveDataLayer`, `ecs`); the editor UI's
// @dcl/inspector re-exports the host surface and leaves @dcl/rpc, ws and
// @dcl/ecs to its own node_modules.
const SERVICE = '@dcl/inspector-service'

// A configured inspector serves the editor UI from its public/ whatever
// happens here; its data-layer host is used when it loads. A package that
// was downloaded rather than installed has no node_modules of its own, so
// its host cannot load: the scene's @dcl/inspector-service (the vendored
// one, or a real install) carries the protocol instead of the whole editor
// failing.
function req(name) {
  if (inspectorRequire) {
    if (name === SERVICE) {
      try {
        return inspectorRequire(inspectorDir)
      } catch (e) {
        const why = String(e && e.message ? e.message : e).split('\n')[0]
        console.error(
          `DCL_ONE_INSPECTOR_DIR: ${inspectorDir} serves the editor UI, but its data-layer host did not load (${why}); using the scene's ${SERVICE} host`
        )
        return sceneRequire(name)
      }
    }
    try {
      return inspectorRequire(name)
    } catch {}
  }
  return sceneRequire(name)
}

// The ecs a host's engines are made of: the service exports its own; the
// editor's host resolves @dcl/ecs beside itself.
function ecsOf(inspector) {
  return inspector.ecs || req('@dcl/ecs/dist-cjs')
}

function pathToPosix(v) {
  return v.replace(/\\/g, '/')
}

function makeFsInterface(workdir) {
  const resolve = (p) => (path.isAbsolute(p) ? p : path.resolve(workdir, p))
  return {
    dirname: (v) => pathToPosix(path.dirname(v)),
    basename: (v) => pathToPosix(path.basename(v)),
    join: (...paths) => path.join(...paths),
    async existFile(filePath) {
      try {
        return (await fs.promises.stat(resolve(filePath))).isFile()
      } catch {
        return false
      }
    },
    async readFile(filePath) {
      return fs.promises.readFile(resolve(filePath))
    },
    async writeFile(filePath, content) {
      const resolved = resolve(filePath)
      await fs.promises.mkdir(path.dirname(resolved), { recursive: true })
      await fs.promises.writeFile(resolved, content)
    },
    async rm(filePath) {
      await fs.promises.rm(resolve(filePath))
    },
    async rmdir(dirPath) {
      await fs.promises.rm(resolve(dirPath), { recursive: true })
    },
    async readdir(dirPath) {
      if (dirPath.indexOf('/../') !== -1) throw new Error('The usage of /../ is not allowed')
      const root = dirPath === '.' || dirPath === './' || dirPath === ''
      const resolved = root ? workdir : dirPath
      const entries = await fs.promises.readdir(resolved)
      return Promise.all(
        entries.map(async (name) => {
          let isDirectory = false
          try {
            isDirectory = (await fs.promises.stat(path.resolve(resolved, name))).isDirectory()
          } catch {}
          return { name: pathToPosix(name), isDirectory }
        })
      )
    },
    cwd: () => pathToPosix(workdir),
    async stat(filePath) {
      const stats = await fs.promises.stat(resolve(filePath))
      return { size: Number(stats.size) }
    }
  }
}

function findComposites(dir, base, out) {
  let entries
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true })
  } catch {
    return out
  }
  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue
    const abs = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'bin' || entry.name === 'dist') continue
      findComposites(abs, base, out)
    } else if (entry.name.endsWith('.composite')) {
      out.push(pathToPosix(path.relative(base, abs)))
    }
  }
  return out
}

async function dumpCrdt() {
  const inspector = req(SERVICE)
  const { Composite, Engine, EntityMappingMode } = ecsOf(inspector)
  // Composites written before the smart-item rename carry asset-packs::
  // component names; the service renames them the way its own composite
  // provider does. An older host without the function reads them as written.
  const rename =
    typeof inspector.renameLegacyComposite === 'function' ? inspector.renameLegacyComposite : (json) => json
  const files = findComposites(projectDir, projectDir, []).sort()
  const composites = {}
  for (const file of files) {
    const raw = await fs.promises.readFile(path.join(projectDir, file), 'utf8')
    composites[file] = Composite.fromJson(rename(JSON.parse(raw)))
  }
  const provider = {
    getCompositeOrNull(src) {
      return src in composites ? { src, composite: composites[src] } : null
    }
  }
  const engine = Engine()
  // NOTE: the crdt this produces carries every component the composites
  // define, but not the editor's own composite::root marker, which a data-layer
  // host adds when it opens a composite. Scene content is identical; only that
  // bookkeeping entry is absent.
  for (const src of Object.keys(composites)) {
    Composite.instance(engine, provider.getCompositeOrNull(src), provider, {
      entityMapping: {
        type: EntityMappingMode.EMM_DIRECT_MAPPING,
        getCompositeEntity: (e) => e
      }
    })
  }
  const crdt = inspector.dumpEngineToCrdtCommands(engine)
  await fs.promises.writeFile(path.join(projectDir, 'main.crdt'), crdt)
  process.stdout.write(JSON.stringify({ ok: true, composites: files.length }) + '\n')
}

async function serve() {
  const inspector = req(SERVICE)
  const fsInterface = makeFsInterface(projectDir)
  const host = await inspector.createDataLayerHost(fsInterface)
  const logToStderr = (...args) => console.error(...args)
  const logger = {
    log: logToStderr,
    info: logToStderr,
    warn: logToStderr,
    error: logToStderr,
    debug: logToStderr
  }
  const port =
    typeof inspector.serveDataLayer === 'function'
      ? (await inspector.serveDataLayer(host, fsInterface, { host: '127.0.0.1', port: 0, logger })).port
      : await serveWithInspectorPackages(inspector, host, fsInterface, logger)
  process.stdout.write(JSON.stringify({ ready: true, port }) + '\n')
  process.stdin.resume()
  process.stdin.on('end', () => process.exit(0))
  process.stdin.on('close', () => process.exit(0))
}

// An editor's host without serveDataLayer: the server is assembled from the
// @dcl/rpc and ws beside it, the way @dcl/sdk-commands does.
function serveWithInspectorPackages(inspector, host, fsInterface, logger) {
  const { createRpcServer } = req('@dcl/rpc')
  const codegen = req('@dcl/rpc/dist/codegen')
  const { WebSocketTransport } = req('@dcl/rpc/dist/transports/WebSocket')
  const { WebSocketServer } = req('ws')
  const rpcServer = createRpcServer({ logger })
  rpcServer.setHandler(async (serverPort) => {
    codegen.registerService(serverPort, inspector.DataServiceDefinition, async () => host.rpcMethods)
  })
  const wss = new WebSocketServer({ host: '127.0.0.1', port: 0 })
  wss.on('connection', (ws) => {
    rpcServer.attachTransport(WebSocketTransport(ws), { fs: fsInterface, engine: host.engine })
    ws.on('error', () => ws.close())
  })
  return new Promise((resolve, reject) => {
    wss.on('listening', () => resolve(wss.address().port))
    wss.on('error', reject)
  })
}

const run = mode === 'dump-crdt' ? dumpCrdt : serve
run().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e))
  process.exit(1)
})
