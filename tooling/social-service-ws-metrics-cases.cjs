#!/usr/bin/env node
// Regenerate only against the clean, pinned source; no source checkout is a build dependency.
// Usage: node tooling/social-service-ws-metrics-cases.cjs <source-checkout> <output.json>
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const { createRequire } = require('node:module')
const revision = 'b6b2cefefdf727b2ed47ddb887e77d226a41eea6'
if (!process.argv[2] || !process.argv[3]) throw Error('Expected source checkout and output JSON')
const source = path.resolve(process.argv[2])
const git = (...args) => execFileSync('git', ['-C', source, ...args], { encoding: 'utf8' }).trim()
if (git('rev-parse', 'HEAD') !== revision || git('status', '--porcelain', '--untracked-files=no')) {
  throw Error('Expected clean pinned source')
}
const req = createRequire(path.join(source, 'package.json'))
req('ts-node').register({ transpileOnly: true, compilerOptions: { module: 'CommonJS', moduleResolution: 'node' } })
const { metricDeclarations } = req(path.join(source, 'src/metrics.ts'))
const { createMetricsComponent } = req('@dcl/metrics')
const name = 'ws_message_size_bytes'
async function main() {
  const metrics = await createMetricsComponent(metricDeclarations, {
    config: { getString: async key => key === 'WKC_METRICS_COLLECT_DEFAULT' ? 'false' : undefined }
  })
  await metrics.start()
  const boundaries = metricDeclarations[name].buckets
  const observations = ['success', 'backpressure', 'dropped'].flatMap(result =>
    [0, ...boundaries.flatMap(bytes => [bytes - 1, bytes, bytes + 1])].map(bytes => ({ result, bytes }))
  )
  for (const { result, bytes } of observations) metrics.observe(name, { result }, bytes)
  const prometheus = (await metrics.registry.metrics()).split('\n')
    .filter(line => line.startsWith(`# HELP ${name} `) || line.startsWith(`# TYPE ${name} `) || line.startsWith(`${name}_`))
    .join('\n') + '\n'
  const files = ['src/metrics.ts', 'node_modules/@dcl/metrics/dist/base.js', 'node_modules/prom-client/lib/histogram.js']
  const capture = {
    baseline: { repository: 'decentraland/social-service-ea', revision },
    runtime: { node: process.version, metrics: req('@dcl/metrics/package.json').version, promClient: req('prom-client/package.json').version },
    scope: 'Actual source metrics component with deterministic byte observations at every declared bucket boundary. No service startup or native socket buffering.',
    files: files.map(file => ({ path: file, sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(source, file))).digest('hex') })),
    observations, prometheus
  }
  fs.writeFileSync(process.argv[3], JSON.stringify(capture, null, 2) + '\n')
  console.log(`Captured ${observations.length} source histogram observations`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
