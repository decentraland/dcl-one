#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/ws-numbers-source-cases.json
// Numeric configuration and timestamp decisions from the pinned source and its
// installed Node/uWS runtime. No socket listener, signing key or media is used.
const { requireNode26, sourceCapture } = require('./social-capture-common.cjs')
requireNode26()
if (!process.argv[2] || !process.argv[3]) throw Error('Usage: node tooling/social-service-ws-number-cases.cjs <upstream-checkout> <output.json>')
const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')
const { spawnSync } = require('node:child_process')
const { req, load, revision } = sourceCapture(process.argv[2])
const { createRecordConfigComponent } = req('@well-known-components/env-config-provider')
const { getWsAuthSettings } = load('utils/ws-session.ts')
const crypto = req(path.join(path.dirname(req.resolve('@dcl/crypto-middleware')), 'verify.js'))
const uwsPath = req.resolve('uWebSockets.js')
const encoded = value => Number.isFinite(value) ? value : String(value)
async function main() {
  const auth = []
  for (const raw of [null,'7','1.9tail','0','-1','Infinity','-Infinity','2147483647','2147483648','300000ms','0x10','NaN','']) {
    try {
      const settings = await getWsAuthSettings({ config:createRecordConfigComponent(raw===null?{}:{WS_AUTH_TIMEOUT_IN_MS:raw,WS_AUTH_SIGNATURE_EXPIRATION_IN_MS:raw}) })
      const timer = setTimeout(()=>{},settings.authTimeoutInMs)
      // Node inserts Timeout objects into an integer millisecond timer bucket.
      const timer_bucket_ms = Math.trunc(timer._idleTimeout)
      clearTimeout(timer)
      auth.push({raw,number:encoded(settings.authTimeoutInMs),signature:encoded(settings.authSignatureExpirationInMs),timer_bucket_ms})
    } catch(error) { auth.push({raw,error:error.message}) }
  }
  const idle=[]
  for (const raw of ['0','0.5','1','7','8','8.9tail','300','960.9','961','65536','65544','4294967304','-65528','-1','Infinity','NaN']) {
    try {
      const number = await createRecordConfigComponent({WS_IDLE_TIMEOUT_IN_SECONDS:raw}).getNumber('WS_IDLE_TIMEOUT_IN_SECONDS')
      const child = spawnSync(process.execPath,['-e',`require(${JSON.stringify(uwsPath)}).App().ws('/*',{idleTimeout:${String(number)}})`],{cwd:os.tmpdir(),encoding:'utf8',timeout:5000})
      if (child.status!==0 && child.signal!=='SIGABRT') throw Error(`Cannot execute native uWS probe: ${child.stderr}`)
      // AppWrapper Int32Value assigns WebSocketBehavior's unsigned-short field.
      idle.push({raw,number:encoded(number),native_allowed:child.status===0,native_seconds:(number|0)&65535})
    } catch(error) { if(error.message.startsWith('Cannot execute')) throw error; idle.push({raw,error:error.message}) }
  }
  const now=1800000000555
  const timestamps=[]
  const actualNow=Date.now
  Date.now=()=>now
  try {
    const rawValues=[null,'',String(now),`${now}.25`,`${now}e0`,`0x${now.toString(16)}`,`0o${now.toString(8)}`,`0b${now.toString(2)}`,` \ufeff${now} `,'0','NaN','Infinity','1_000','0x','+0x10','1e999','0x20000000000003','0x100000000000007','0b1'+'0'.repeat(52)+'11','0o1000000000000000003']
    for(const raw of rawValues) for(const window of [0,0.5,1.5,Infinity,-Infinity]) {
      try {
        const parsed=crypto.verifyTimestamp(raw===null?undefined:raw)
        crypto.verifyExpiration(parsed,{expiration:window})
        timestamps.push({raw,window:encoded(window),accepted:true,parsed})
      } catch(error) {timestamps.push({raw,window:encoded(window),accepted:false,error:error.message})}
    }
  } finally {Date.now=actualNow}
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},
    runtime:{node:process.version,uws_source_commit:fs.readFileSync(path.join(path.dirname(uwsPath),'source_commit'),'utf8').trim()},
    scope:'Actual env provider/getWsAuthSettings/Node timer buckets, native uWS idle admission, crypto middleware timestamp+expiry functions. Native idle seconds follow binding integer conversion; cryptographic signatures and wall-clock timeout scheduling are not exercised by this capture.',now,auth,idle,timestamps},null,2)+'\n')
  process.stdout.write(`Captured auth${auth.length}, idle${idle.length}, timestamp${timestamps.length} source decisions\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
