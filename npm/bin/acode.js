#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const { ensureBinary } = require('../lib/installer');

let binaryPath = ensureBinary('acode');

if (!binaryPath) {
  console.error('[FAIL] acode native binary not found.');
  console.error('[FAIL] Please install Rust toolchain and run: cargo install --path .');
  process.exit(1);
}

// Ensure execution permissions on Unix
if (process.platform !== 'win32') {
  try {
    fs.chmodSync(binaryPath, 0o755);
  } catch {
    // Ignore permission error if already executable
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
