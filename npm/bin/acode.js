#!/usr/bin/env node

const fs = require('fs');
const { spawn } = require('child_process');
const { ensureBinary } = require('../lib/installer');

(async () => {
  let binaryPath = await ensureBinary('acode');

  if (!binaryPath) {
    console.error('[FAIL] acode native engine could not be found or downloaded.');
    console.error('[FAIL] You can download precompiled binaries from:');
    console.error('       https://github.com/AtlasResearch-Labs/acode-cli/releases');
    process.exit(1);
  }

  // Ensure execution permissions on Unix / Termux
  if (process.platform !== 'win32') {
    try {
      fs.chmodSync(binaryPath, 0o755);
    } catch {
      // Ignore permission error if already executable or read-only
    }
  }

  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
    windowsHide: false,
  });

  child.on('error', (err) => {
    console.error('[FAIL] Failed to launch acode process:', err.message);
    process.exit(1);
  });

  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code ?? 0);
    }
  });
})();
