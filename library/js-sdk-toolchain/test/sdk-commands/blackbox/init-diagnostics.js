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
        // Retain at most 4096 characters. Emit only recognized npm error codes, never raw output.
        const codes = () => {
          const matches = [...tail.matchAll(/npm (?:ERR!|error) code ([A-Z0-9_]+)\b/g)]
          return { npmCodes: [...new Set(matches.map((match) => match[1]).filter((code) => safeCodes.has(code)))] }
        }
        const finish = begin('dependency-child', codes)
        let child
        try {
          child = spawn(...args)
        } catch (error) {
          finish('failed', safeCodes.has(error?.code) ? { code: error.code } : {})
          throw error
        }
        const collect = (data) => {
          tail = (tail + data.toString()).slice(-4096)
        }
        child.stderr?.on('data', collect)
        child.once('close', (code, signal) => {
          child.stderr?.off('data', collect)
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
