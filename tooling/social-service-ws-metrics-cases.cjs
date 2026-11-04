#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/ws-metrics-source-cases.json
// Regenerate only against the clean, pinned source; no source checkout is a build dependency.
// Usage: node tooling/social-service-ws-metrics-cases.cjs <source-checkout> <output.json>
const { requireNode26, sourceCapture, sha256 } = require('./social-capture-common.cjs')
requireNode26()
const fs = require('node:fs')
const path = require('node:path')
if (!process.argv[2] || !process.argv[3]) throw Error('Expected source checkout and output JSON')
const { upstream: source, revision, req } = sourceCapture(process.argv[2])
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
    files: files.map(file => ({ path: file, sha256: sha256(source, file) })),
    observations, prometheus
  }
  fs.writeFileSync(process.argv[3], JSON.stringify(capture, null, 2) + '\n')
  console.log(`Captured ${observations.length} source histogram observations`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
