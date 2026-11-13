import { constants } from 'node:fs'
import { access, readFile, stat } from 'node:fs/promises'
import { spawn } from 'node:child_process'
import { Buffer } from 'node:buffer'
import { setTimeout, clearTimeout } from 'node:timers'
import os from 'node:os'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const unknownDirectory = () => ({ directory: null, readable: null, writable: null })

async function selectedBrowser(candidates) {
  const { findBrowser } = await import('../library/ui/figma/review/scripts/capture-browser.mjs')
  return findBrowser(candidates)
}

function versionOutput(executable) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, ['--version'], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true
    })
    const stdout = child.stdout
    let deadline
    let settled = false
    let size = 0
    const chunks = []
    const finish = (ok) => {
      if (settled) return
      settled = true
      clearTimeout(deadline)
      stdout?.removeListener('data', onData)
      stdout?.destroy()
      if (!ok && Number.isInteger(child.pid) && child.pid > 0) child.kill('SIGKILL')
      child.unref()
      if (ok) resolve({ stdout: Buffer.concat(chunks).toString('utf8') })
      else reject(new Error('Browser version metadata unavailable'))
    }
    const onData = (chunk) => {
      size += chunk.length
      if (size > 4096) finish(false)
      else chunks.push(chunk)
    }
    // Own the deadline and pipe lifetime: a child may ignore SIGTERM or leave a writer behind.
    child.once('error', () => finish(false))
    child.once('close', (code) => finish(code === 0))
    if (!stdout) {
      finish(false)
      return
    }
    deadline = setTimeout(() => finish(false), 3000)
    stdout.on('data', onData)
    stdout.once('error', () => finish(false))
  })
}

function parsedVersion(value) {
  if (typeof value !== 'string') return null
  const match = value
    .trim()
    .match(
      /^(Google Chrome(?: for Testing)?|Chromium|Brave Browser) ([0-9]{1,6}(?:\.[0-9]{1,6}){1,3})$/
    )
  return match ? { family: match[1], version: match[2] } : null
}

function pressureSome10s(value) {
  if (typeof value !== 'string' || value.length > 8192 || !value.trim()) return null
  const kinds = new Set()
  let some
  for (const line of value.trim().split('\n')) {
    const match = line.match(
      /^(some|full) avg10=([0-9]+(?:\.[0-9]+)?) avg60=([0-9]+(?:\.[0-9]+)?) avg300=([0-9]+(?:\.[0-9]+)?) total=[0-9]+$/
    )
    if (!match || kinds.has(match[1])) return null
    const averages = match.slice(2, 5).map(Number)
    if (averages.some((value) => !Number.isFinite(value) || value > 100)) return null
    kinds.add(match[1])
    if (match[1] === 'some') some = averages[0]
  }
  return some === undefined ? null : some > 0
}

async function directoryObservation(directory, operations) {
  if (!directory) return unknownDirectory()
  try {
    if (!(await operations.stat(directory)).isDirectory())
      return { directory: false, readable: null, writable: null }
  } catch (error) {
    return error?.code === 'ENOENT'
      ? { directory: false, readable: null, writable: null }
      : unknownDirectory()
  }
  const permitted = async (mode) => {
    try {
      await operations.access(directory, mode | constants.X_OK)
      return true
    } catch (error) {
      return ['EACCES', 'EPERM', 'EROFS'].includes(error?.code) ? false : null
    }
  }
  return {
    directory: true,
    readable: await permitted(constants.R_OK),
    writable: await permitted(constants.W_OK)
  }
}

function integerObservation(read) {
  try {
    const value = read()
    return Number.isInteger(value) && value >= 0 && value <= 0xffffffff ? value : null
  } catch {
    return null
  }
}

// Probe operations are explicit so failures can be tested without altering the process environment.
export async function collectBrowserEnvironment(probes = {}) {
  const operations = {
    env: process.env,
    home: () => os.homedir(),
    uid: () => process.getuid?.(),
    euid: () => process.geteuid?.(),
    findBrowser: selectedBrowser,
    version: versionOutput,
    stat,
    access,
    readFile,
    ...probes
  }
  let home = null
  try {
    const value = operations.home()
    if (typeof value === 'string' && path.isAbsolute(value)) home = value
  } catch {
    // Missing private-path information stays unknown; it is never guessed or logged.
  }
  const configuredCache = operations.env.XDG_CACHE_HOME
  const cacheRoot = configuredCache
    ? typeof configuredCache === 'string' && path.isAbsolute(configuredCache)
      ? configuredCache
      : null
    : home && path.join(home, '.cache')
  const report = {
    browser: { selected: null, family: null, version: null },
    user: { uid: integerObservation(operations.uid), euid: integerObservation(operations.euid) },
    cache: {},
    pressure: {}
  }
  try {
    const executable = await operations.findBrowser([
      operations.env.BROWSER_PATH,
      operations.env.CHROME_PATH,
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/usr/bin/google-chrome',
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser'
    ])
    if (typeof executable === 'string' && executable) {
      report.browser.selected = true
      try {
        const result = parsedVersion((await operations.version(executable))?.stdout)
        if (result) Object.assign(report.browser, result)
      } catch {
        // Subprocess stdout, stderr and error objects can contain private paths or configuration.
      }
    }
  } catch {
    // An unavailable selection is diagnostic information, not a replacement for the actual test gate.
  }
  for (const [label, directory] of [
    ['home', home],
    ['xdgRoot', cacheRoot],
    ['fontconfigSystem', '/var/cache/fontconfig'],
    ['fontconfigUser', cacheRoot && path.join(cacheRoot, 'fontconfig')],
    ['fontconfigLegacy', home && path.join(home, '.fontconfig')]
  ])
    report.cache[label] = await directoryObservation(directory, operations)
  for (const [label, file] of [
    ['cpuSome10s', '/proc/pressure/cpu'],
    ['memorySome10s', '/proc/pressure/memory'],
    ['ioSome10s', '/proc/pressure/io']
  ]) {
    try {
      report.pressure[label] = pressureSome10s(await operations.readFile(file, 'utf8'))
    } catch {
      report.pressure[label] = null
    }
  }
  return report
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  let report
  try {
    report = await collectBrowserEnvironment()
  } catch {
    report = { status: 'unknown' }
  }
  process.stdout.write(JSON.stringify(report) + '\n')
}
