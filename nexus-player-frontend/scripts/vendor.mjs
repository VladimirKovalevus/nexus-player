// Installs and builds vendor/butterchurn. Usage: node scripts/vendor.mjs [setup]
// "setup" always installs and builds; no argument does each step only when missing.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const vendorDir = path.resolve(here, '..', '..', 'vendor', 'butterchurn');
const rollupBin = path.join(vendorDir, ...'node_modules/rollup/dist/bin/rollup'.split('/'));
const bundle = path.join(vendorDir, 'dist', 'butterchurn.min.js');
const presetsAlias = path.join(vendorDir, 'node_modules', 'butterchurn-presets-js');

const force = process.argv[2] === 'setup';

function fail(msg) {
  console.error(`vendor: ${msg}`);
  process.exit(1);
}

function run(label, args, env) {
  const res = spawnSync(process.execPath, args, {
    cwd: vendorDir,
    stdio: 'inherit',
    env: { ...process.env, ...env },
  });
  if (res.error) fail(`${label} failed to start: ${res.error.message}`);
  if (res.status !== 0) fail(`${label} failed (exit ${res.status ?? res.signal})`);
}

if (force || !existsSync(rollupBin) || !existsSync(presetsAlias)) {
  const npmCli = process.env.npm_execpath;
  if (!npmCli) fail('npm_execpath is not set; run this through an npm script (npm run setup).');
  console.log('vendor: installing butterchurn dependencies');
  run('npm ci', [npmCli, 'ci'], { PUPPETEER_SKIP_DOWNLOAD: 'true' });
}

if (force || !existsSync(bundle)) {
  if (!existsSync(rollupBin)) fail('rollup is missing; run npm run setup.');
  console.log('vendor: building butterchurn');
  run('rollup', [rollupBin, '-c'], { NODE_ENV: 'production' });
  if (!existsSync(bundle)) fail('build finished but dist/butterchurn.min.js is missing');
}
