/**
 * Atlas Daedalus (`acode-cli`) — Programmatic API & Native Launcher
 * (C) 2026 Atlas Research Labs — MIT License
 */

const path = require('path');
const { ensureBinary, resolveExistingBinary, getPlatformKey } = require('./lib/installer');

module.exports = {
  ensureBinary,
  resolveExistingBinary,
  getPlatformKey,
  getBinaryPath: () => ensureBinary('acode'),
  binDir: path.join(__dirname, 'bin'),
  defaultBinaryName: process.platform === 'win32' ? 'acode.exe' : 'acode'
};
