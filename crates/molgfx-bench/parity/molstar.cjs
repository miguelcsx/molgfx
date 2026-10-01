// Real Chromium/WebGL2 transport; no headless-gl, mocked DOM, or raster fallback.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const http = require('node:http');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const esbuild = require('esbuild');
const puppeteer = require('puppeteer-core');
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');

async function main() {
    const request = JSON.parse(fs.readFileSync(process.env.MOLGFX_PARITY_REQUEST, 'utf8'));
    const root = fs.realpathSync(process.env.MOLSTAR_ROOT);
    const source = path.join(root, 'src');
    if (!fs.statSync(source).isDirectory()) throw new Error('MOLSTAR_ROOT must be the local source checkout, not a registry package');
    const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
    const bytes = fs.readFileSync(request.path);
    if (sha256(bytes) !== request.fixture.sha256) throw new Error('Fixture SHA-256 differs from the catalog; execution refused');
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'molstar-parity-browser-'));
    let browser, server;
    const connected = Boolean(process.env.MOLSTAR_CDP_ENDPOINT);
    try {
        const bundle = path.join(temporary, 'adapter.js');
        const result = await esbuild.build({ entryPoints: [path.join(__dirname, 'molstar-browser.cjs')],
            bundle: true, outfile: bundle, platform: 'browser', metafile: true,
            tsconfig: path.join(root, 'tsconfig.json'), target: 'es2018',
            nodePaths: (process.env.NODE_PATH || '').split(path.delimiter).filter(Boolean),
            external: ['crypto', 'fs', 'path', 'stream'],
            define: { 'process.env.NODE_ENV': '"production"', 'process.env.DEBUG': 'false',
                __MOLSTAR_PLUGIN_VERSION__: JSON.stringify(version), __MOLSTAR_BUILD_TIMESTAMP__: '0' },
            plugins: [{ name: 'local-molstar-source', setup(build) {
                build.onResolve({ filter: /^@molstar\// }, args => {
                    const name = path.join(source, args.path.slice('@molstar/'.length));
                    return { path: fs.existsSync(name + '.ts') ? name + '.ts' : path.join(name, 'index.ts') };
                });
            } }] });
        const inputs = Object.keys(result.metafile.inputs).map(name => path.resolve(name)).filter(name => name.startsWith(source + path.sep)).sort();
        const sourceDigest = sha256(Buffer.from(inputs.map(name => `${path.relative(root, name)}:${sha256(fs.readFileSync(name))}`).join('\n')));
        let revision = null;
        try { revision = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim(); } catch { /* Source hashes remain authoritative for a copied local checkout. */ }
        server = http.createServer((req, res) => {
            const routes = { '/request.json': ['application/json', JSON.stringify(request)],
                '/fixture': ['application/octet-stream', bytes], '/adapter.js': ['text/javascript', fs.readFileSync(bundle)],
                '/': ['text/html', '<!doctype html><html><head><link rel="icon" href="data:,"><title>Mol* parity completed output</title></head><body style="margin:0;background:black"><div id="viewport" style="position:relative"><canvas id="molstar"></canvas></div><script src="/adapter.js"></script></body></html>'] };
            const item = routes[req.url];
            if (!item) { res.writeHead(404).end(); return; }
            res.setHeader('Content-Type', item[0]);
            res.end(item[1]);
        });
        await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
        if (connected) browser = await puppeteer.connect({ browserWSEndpoint: process.env.MOLSTAR_CDP_ENDPOINT, protocolTimeout: 0 });
        else {
            if (!process.env.MOLSTAR_CHROMIUM) throw new Error('Set MOLSTAR_CHROMIUM to a real Chromium executable, or MOLSTAR_CDP_ENDPOINT to an isolated running Chromium');
            browser = await puppeteer.launch({ executablePath: process.env.MOLSTAR_CHROMIUM, headless: true,
                protocolTimeout: 0, args: ['--force-device-scale-factor=1'] });
        }
        const page = await browser.newPage();
        try {
            const [width, height] = request.catalog.extent;
            await page.setViewport({ width, height, deviceScaleFactor: 1 });
            const messages = [], errors = [];
            page.on('console', message => messages.push({ type: message.type(), text: message.text() }));
            page.on('pageerror', error => errors.push(String(error)));
            await page.goto(`http://127.0.0.1:${server.address().port}/`, { waitUntil: 'load' });
            const response = await page.evaluate(async () => {
                const request = await (await fetch('/request.json')).json();
                const bytes = new Uint8Array(await (await fetch('/fixture')).arrayBuffer());
                return window.executeMolstarParity(request, bytes);
            });
            if (errors.length || messages.some(message => message.type === 'error')) throw new Error(`Browser rendering errors: ${JSON.stringify({ errors, messages })}`);
            const png = response.png_data_url;
            delete response.png_data_url;
            response.engine_version = version;
            response.provenance = { source_root: root, git_revision: revision, source_inputs: inputs.length,
                source_inputs_sha256: sourceDigest, bundle_sha256: sha256(fs.readFileSync(bundle)),
                chromium: await browser.version(), transport: 'real-browser-WebGL2', node: process.version,
                source_sha256: sha256(bytes), adapter_sha256: sha256(fs.readFileSync(__filename)),
                browser_adapter_sha256: sha256(fs.readFileSync(path.join(__dirname, 'molstar-browser.cjs'))) };
            response.browser_console = messages;
            if (png) {
                const encoded = Buffer.from(png.slice('data:image/png;base64,'.length), 'base64');
                fs.writeFileSync(path.join(request.output, 'image.png'), encoded);
                response.image_metadata = { width: encoded.readUInt32BE(16), height: encoded.readUInt32BE(20),
                    sha256: sha256(encoded), bytes: encoded.length, encoding: 'PNG from final completed ImagePass readPixels; no additional render' };
            }
            fs.writeFileSync(request.response, JSON.stringify(response, null, 2));
        } finally { await page.close(); }
    } finally {
        if (browser) { if (connected) browser.disconnect(); else await browser.close(); }
        if (server) await new Promise(resolve => server.close(resolve));
        fs.rmSync(temporary, { recursive: true, force: true });
    }
}
main().catch(error => { console.error(error.stack); process.exitCode = 1; });
