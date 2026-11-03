// Run from the repository with Node and OpenSSL. OPENSSL_BIN can name a local OpenSSL binary.
import { execFileSync, spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { createSecureServer } from 'node:http2'
import { createServer } from 'node:https'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const directory = await mkdtemp(join(tmpdir(), 'explorer-http2-'))
const servers = []
try {
  const key = join(directory, 'key.pem')
  const cert = join(directory, 'cert.pem')
  execFileSync(process.env.OPENSSL_BIN || 'openssl', [
    'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert,
    '-days', '1', '-subj', '/CN=localhost'
  ], { stdio: 'ignore' })
  const tls = { key: await readFile(key), cert: await readFile(cert) }
  const reply = (_request, response) => response.end('ready')
  servers.push(createSecureServer(tls, reply), createServer(tls, reply))
  for (const server of servers) {
    server.listen(0, '127.0.0.1')
    await once(server, 'listening')
  }
  const name = 'http_client::tests::loopback_negotiates_http2_and_http11'
  // CI already built the whole workspace in its test profile. Reuse that test binary.
  let command = 'cargo'
  let args = ['test', '--locked', '-p', 'platform', '--lib', name, '--', '--ignored', '--exact']
  if (process.argv[2]) {
    const artifacts = (await readFile(process.argv[2], 'utf8')).trim().split('\n').map(line => JSON.parse(line))
    const binary = artifacts.find(a => a.target?.name === 'platform' && a.profile?.test && a.executable)
    if (!binary) throw Error('compiled platform test executable missing')
    command = binary.executable
    args = [name, '--ignored', '--exact']
  }
  const cargo = spawn(command, args, {
    cwd: new URL('../rendering/bevy-explorer/source/', import.meta.url),
    env: { ...process.env, DCL_TEST_H2_PORT: servers[0].address().port, DCL_TEST_H1_PORT: servers[1].address().port },
    stdio: ['ignore', 'pipe', 'inherit']
  })
  let output = ''
  cargo.stdout.on('data', chunk => { output += chunk; process.stdout.write(chunk) })
  const [code] = await once(cargo, 'close')
  if (code !== 0 || !output.includes(`test ${name} ... ok`)) throw Error('HTTP negotiation regression did not pass')
} finally {
  for (const server of servers) server.close()
  await rm(directory, { recursive: true, force: true })
}
