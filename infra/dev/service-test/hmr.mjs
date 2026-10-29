// Connects to the Vite HMR socket the way the browser client does, registers
// /src/main.js in the module graph, then waits for its hot update.
const [port] = process.argv.slice(2);
const base = `http://127.0.0.1:${port}`;
setTimeout(() => { console.log('timeout'); process.exit(1); }, 300_000);
const client = await (await fetch(`${base}/@vite/client`)).text();
const token = client.match(/wsToken = "([^"]*)"/)?.[1] ?? '';
const socket = new WebSocket(`ws://127.0.0.1:${port}/?token=${token}`, 'vite-hmr');
socket.addEventListener('message', ({ data }) => {
  const payload = JSON.parse(data);
  if (payload.type === 'update' && payload.updates.some((u) => u.path === '/src/main.js')) {
    console.log('hmr update /src/main.js');
    process.exit(0);
  }
});
socket.addEventListener('open', async () => {
  await (await fetch(`${base}/src/main.js`)).text();
  console.log('ready');
});
socket.addEventListener('close', () => { console.log('closed'); process.exit(1); });
