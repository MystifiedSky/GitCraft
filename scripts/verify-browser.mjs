import { createServer } from '../ui/node_modules/vite/dist/node/index.js';
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join, dirname, basename } from 'node:path';
const repository = resolve(import.meta.dirname, '..');
const executable = process.env.CHROME_PATH ?? ['C:/Program Files/Google/Chrome/Application/chrome.exe', '/usr/bin/google-chrome', '/usr/bin/chromium'].find(existsSync);
if (!executable) throw new Error('Set CHROME_PATH to an installed Chromium browser.');
const server = await createServer({ root: join(repository, 'ui'), configFile: join(repository, 'ui/vite.config.ts'), server: { host: '127.0.0.1', port: 0, open: false } });
await server.listen();
const temporaryRoot = resolve(tmpdir());
const profile = mkdtempSync(join(temporaryRoot, 'gitcraft-headless-'));
const child = spawn(executable, ['--headless=new', '--disable-gpu', '--no-first-run', '--disable-background-networking', '--disable-sync', '--remote-debugging-port=0', '--remote-debugging-address=127.0.0.1', `--user-data-dir=${profile}`, 'about:blank'], { windowsHide: true, stdio: 'ignore' });
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let socket;
try {
  const portFile = join(profile, 'DevToolsActivePort');
  for (let i = 0; i < 100 && !existsSync(portFile); i++) await sleep(100);
  const port = readFileSync(portFile, 'utf8').split('\n')[0];
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket = new WebSocket(pages.find(page => page.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let next = 0; const pending = new Map(); const errors = [];
  socket.onmessage = event => { const message = JSON.parse(event.data); if (message.id) { pending.get(message.id)?.(message); pending.delete(message.id); } else if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails); };
  const send = (method, params = {}) => new Promise(resolve => { const id = ++next; pending.set(id, resolve); socket.send(JSON.stringify({id,method,params})); });
  await send('Runtime.enable'); await send('Page.enable');
  await send('Page.navigate', {url: `http://127.0.0.1:${server.httpServer.address().port}/tests/browser/index.html`});
  let report;
  for (let i = 0; i < 600; i++) {
    await sleep(200);
    const value = await send('Runtime.evaluate', {expression: 'document.getElementById("verification-results")?.textContent', returnByValue: true});
    if (value.result?.result?.value) { report = JSON.parse(value.result.result.value); break; }
  }
  mkdirSync(join(repository, 'target/verification'), {recursive: true});
  writeFileSync(join(repository, 'target/verification/browser-errors.json'), JSON.stringify(errors, null, 2));
  if (!report) throw new Error('Headless verification timed out; see target/verification/browser-errors.json');
  writeFileSync(join(repository, 'target/verification/browser-report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  if (report.failed) process.exitCode = 1;
} finally {
  socket?.close(); child.kill(); await server.close();
  // Chrome owns this isolated directory until its shutdown has completed.
  await sleep(500); try { if (dirname(profile) === temporaryRoot && basename(profile).startsWith("gitcraft-headless-")) rmSync(profile, {recursive:true,force:true}); } catch { /* A late browser process can retain its temporary profile. */ }
}
