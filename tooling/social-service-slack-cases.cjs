#!/usr/bin/env node
// Actual source payload builders, Slack component and installed SDK. The axios
// adapter is in-process; no Slack message or other external request is sent.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { req, load, logs, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const output = process.argv[3]
const { WebClient } = req('@slack/web-api')
const policy = req('@slack/web-api/dist/retry-policies').tenRetriesInAboutThirtyMinutes
const originalRequest = WebClient.prototype.makeRequest
let scenario, attempts
WebClient.prototype.makeRequest = function (...args) {
  // Preserve source retry decisions/counts while making test waits <=1ms.
  this.retryConfig = { ...policy, minTimeout: 0, maxTimeout: 1 }
  this.axios.defaults.adapter = async config => {
    attempts.push({ method: config.method, path: config.url, authorization: config.headers.Authorization,
      content_type: config.headers['Content-Type'], body: config.data })
    if (scenario === 'transport-then-success' && attempts.length === 1) throw Object.assign(Error('fixture transport error'), { request: {} })
    const first = attempts.length === 1
    const status = scenario === 'redirect' ? 302 : scenario === 'exhausted' || (scenario === 'http-then-success' && first) ? 500
      : (scenario.startsWith('rate-') && first) ? 429 : 200
    return { status, statusText: status === 200 ? 'OK' : 'fixture failure',
      headers: scenario === 'rate-then-success' && first ? { 'retry-after': '0' } : {},
      data: { ok: scenario !== 'platform-error', error: 'fixture_platform_error' },
      config, request: { path: '/api/chat.postMessage' } }
  }
  return originalRequest.apply(this, args)
}
async function main() {
  const component = req('@dcl/slack-component').createSlackComponent({ logs }, { token: 'fixture-token' })
  const messages = load('utils/slackMessages.ts')
  const referrer = '0x9111111111111111111111111111111111111111'
  const invited = '0x9222222222222222222222222222222222222222'
  const dashboard = 'https://dashboard.example.org/referrals'
  const newest = 1767268801234, previous = 1767268680000
  const cases = []
  for (const isDev of [true, false]) {
    for (const kind of ['timing', 'ip', 'milestone']) {
      const message = kind === 'timing' ? messages.referralSuspiciousTimingMessage(referrer, String(newest), String(previous), 2.02,
        new Date(newest).toISOString(), new Date(previous).toISOString(), isDev, dashboard)
        : kind === 'ip' ? messages.referralIpMatchRejectionMessage(referrer, invited, '192.0.2.1', isDev, dashboard, 2)
        : messages.referral100InvitesReachedMessage(referrer, isDev, dashboard)
      scenario = 'success'; attempts = []
      await component.sendMessage(message)
      cases.push({ kind, is_dev: isDev, referrer, invited, dashboard, newest_ms: newest, previous_ms: previous, message, request: attempts[0] })
    }
  }
  const transport = []
  for (scenario of ['success', 'platform-error', 'http-then-success', 'rate-then-success', 'rate-missing-header', 'transport-then-success', 'exhausted', 'redirect']) {
    attempts = []
    let result = 'ok'
    try { await component.sendMessage(cases[0].message) } catch { result = 'error' }
    transport.push({ scenario, attempts: attempts.length, result })
  }
  const originalRandom = Math.random
  let retryTimeouts
  try { Math.random = () => 0.5; retryTimeouts = req('retry').timeouts(policy) } finally { Math.random = originalRandom }
  const sources = ['src/utils/slackMessages.ts', 'src/logic/referral/referral.ts',
    'node_modules/@dcl/slack-component/dist/src/component.js', 'node_modules/@slack/web-api/dist/WebClient.js',
    'node_modules/@slack/web-api/dist/retry-policies.js', 'node_modules/retry/lib/retry.js']
  fs.writeFileSync(output, JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision,
    sources: sources.map(file => ({ path: file, sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream, file))).digest('hex') })) },
    runtime: { node: process.version, slack_sdk: req('@slack/web-api/package.json').version },
    scope: 'Actual source Slack payloads/component/SDK with an in-process axios adapter. Form/auth fields and retry classifications/counts are executed; transport test waits are reduced to <=1ms. Default retry durations are separately captured with deterministic Math.random=0.5. No external Slack calls, deployed success or equality beyond the existing Outbound timeouts/guards is claimed.',
    cases, transport, retry_timeout_ms_at_random_half: retryTimeouts }, null, 2) + '\n')
  process.stdout.write(`Captured ${cases.length} Slack payloads and ${transport.length} SDK transport cases\n`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
