// Plays the page of a run with the real sender (deploy/web/engine/perf_stream.js, copied beside this
// script as perf_stream.mjs), against the runner's server. What it appended to each stream is written to
// <dir>/expected.csv and <dir>/expected.log for the test to compare with what the server stored.
//
//   node sender.mjs <origin> <perfStream path> <scenario> <dir>
//
// ok       a measured run: a header, 3000 rows and some log lines, then end with exit code 0
// panic    the same cut short: as the engine's panic hook does, what it still held is appended, then the
//          panic's own block, which the engine formats and scrubs, and the run ends with exit code 101
// giveup   more CSV than the server's file cap holds: the sender gives the stream up and still ends the run
import { writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { createPerfStream } from './perf_stream.mjs'

const [origin, path, scenario, dir] = process.argv.slice(2)

const expected = { csv: '', log: '' }
const instantly = () => Promise.resolve()
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const tail = { csv: '9001,16.500,0\r\n', log: 'lines the engine had not handed over\n' }
// the panic's block as the engine hands it over (crates/perf_output `layer::panic_block`)
const panicBlock =
  '12:00:00.123 - <color=#FF5C5C>[panic]</color>: \n' +
  'PanicException: index out of bounds\n' +
  '    second line\n' +
  'DebugLogReportHandler:LogExceptionInternal\n'

const stream = createPerfStream({ path, csv: true }, { origin, wait: instantly })
const append = (name, text) => {
  expected[name] += text
  stream.append(name, text)
}

const header = '"Frame","CPU Time","GPU Time"\r\n'
const rows = (from, to) => {
  let text = ''
  for (let frame = from; frame <= to; frame++) text += `${frame},${(16 + (frame % 7) / 10).toFixed(3)},0\r\n`
  return text
}

append('log', '12:00:00.000 - <color=#8CC8FF>[autopilot]</color>: signed in (é — 日本語 in a log line)\n')
append('csv', header)

const frames = scenario === 'panic' ? 900 : 3000
for (let frame = 1; frame <= frames; frame += 100) {
  append('csv', rows(frame, frame + 99))
  if (frame % 500 === 1) append('log', `12:00:${String(frame % 60).padStart(2, '0')}.000 - <color=#8CC8FF>[autopilot]</color>: frame ${frame}\n`)
  await sleep(2)
}

if (scenario === 'panic') {
  // the engine's panic hook: what it still held first, then the panic as one exception block
  append('csv', tail.csv)
  append('log', tail.log)
  expected.log += panicBlock
  await stream.panicked(panicBlock)
} else {
  append('log', '12:01:30.000 - <color=#8CC8FF>[autopilot]</color>: [ExitUtils] Exit requested at 2026-10-27T12:01:30Z\n')
  await stream.end(0)
}

writeFileSync(join(dir, 'expected.csv'), expected.csv)
writeFileSync(join(dir, 'expected.log'), expected.log)
// give the connections a moment to settle: node on Windows can abort when it exits with one closing
await sleep(200)
process.exit(0)
