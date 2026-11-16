const { basename } = require('node:path')
const { constants } = require('node:os')

const stages = new Set([
  'components',
  'fixture',
  'init',
  'build',
  'template-fetch',
  'template-body',
  'template-write',
  'dependency-install',
  'dependency-child'
])
const safeCodes = new Set([
  'ECONNRESET',
  'ECONNREFUSED',
  'ETIMEDOUT',
  'ENOTFOUND',
  'EAI_AGAIN',
  'EACCES',
  'EPERM',
  'ENOSPC',
  'ERESOLVE',
  'E404',
  'E401',
  'E403',
  'EINTEGRITY'
])

function createInitDiagnostics(write = console.error, now = () => performance.now()) {
  const pending = new Map()

  function emit(stage, state, start, details = {}) {
    try {
      write(JSON.stringify({ stage, state, elapsedMs: Math.round(now() - start), ...details }))
    } catch {
      // A diagnostic reporter must not change command success or failure.
    }
  }

  function begin(stage, details) {
    if (!stages.has(stage)) throw new Error('Unknown init diagnostic stage')
    const start = now()
    pending.set(stage, { start, details })
    emit(stage, 'start', start)
    return (state, result) => {
      pending.delete(stage)
      emit(stage, state, start, result)
    }
  }

  async function step(stage, action) {
    const finish = begin(stage)
    try {
      const result = await action()
      finish('complete')
      return result
    } catch (error) {
      finish('failed', safeCodes.has(error?.code) ? { code: error.code } : {})
      throw error
    }
  }

  return {
    step,
    reportPending() {
      for (const [stage, { start, details }] of pending) emit(stage, 'pending', start, details?.())
    },
    wrapFetch(fetcher) {
      return {
        async fetch(...args) {
          const response = await step('template-fetch', () => fetcher.fetch(...args))
          const arrayBuffer = response.arrayBuffer.bind(response)
          response.arrayBuffer = (...bodyArgs) => step('template-body', () => arrayBuffer(...bodyArgs))
          return response
        }
      }
    },
    wrapFs(fs) {
      return {
        ...fs,
        writeFile(...args) {
          const writeFile = () => fs.writeFile(...args)
          return basename(String(args[0])) === 'temp-zip-project.zip' ? step('template-write', writeFile) : writeFile()
        }
      }
    },
    wrapSpawn(spawn) {
      return (...args) => {
        let tail = ''
        const observedCodes = new Set()
        // Receipts contain only recognized codes; exec separately forwards raw child output.
        const collectCodes = (text) => {
          for (let offset = 0; offset < text.length; offset += 1024) {
            const window = tail + text.slice(offset, offset + 1024)
            for (const match of window.matchAll(/npm (?:ERR!|error) code ([A-Z0-9_]+)(?=[^A-Z0-9_])/g)) {
              if (safeCodes.has(match[1])) observedCodes.add(match[1])
            }
            // Enough overlap for the longest recognized receipt, including split chunks.
            tail = window.slice(-64)
          }
        }
        const codes = () => ({ npmCodes: [...observedCodes] })
        const finish = begin('dependency-child', codes)
        let child
        try {
          child = spawn(...args)
        } catch (error) {
          finish('failed', safeCodes.has(error?.code) ? { code: error.code } : {})
          throw error
        }
        const collect = (data) => {
          collectCodes(data.toString())
        }
        child.stderr?.on('data', collect)
        child.once('close', (code, signal) => {
          child.stderr?.off('data', collect)
          collectCodes('\n')
          finish('close', {
            exitCode: Number.isInteger(code) ? code : null,
            signal: Object.hasOwn(constants.signals, signal) ? signal : null,
            ...(code !== 0 ? codes() : {})
          })
          tail = ''
        })
        return child
      }
    }
  }
}

function getInitSceneDirectory(mode) {
  return mode === 'isolated' ? 'tmp/blackbox-scene-isolated' : 'tmp/blackbox-scene'
}

module.exports = { createInitDiagnostics, getInitSceneDirectory }
