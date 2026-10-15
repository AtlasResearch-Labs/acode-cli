const fs = require('fs');
const path = require('path');

function getBinaryName(base) {
  return process.platform === 'win32' ? `${base}.exe` : base;
}

function resolveExistingBinary(base) {
  const binaryName = getBinaryName(base);

  // 1. Packaged native binary in npm/bin/native/
  const localNative = path.join(__dirname, '..', 'bin', 'native', binaryName);
  if (fs.existsSync(localNative)) {
    return localNative;
  }

  // 2. Direct package bin directory
  const directBin = path.join(__dirname, '..', 'bin', binaryName);
  if (fs.existsSync(directBin)) {
    return directBin;
  }

  // 3. System PATH or Cargo bin
  const homeDir = process.env.HOME || process.env.USERPROFILE || '';
  const cargoBin = path.join(homeDir, '.cargo', 'bin', binaryName);
  if (fs.existsSync(cargoBin)) {
    return cargoBin;
  }

  // 4. Workspace development build: target/release or target/debug
  const workspaceRelease = path.resolve(__dirname, '..', '..', 'target', 'release', binaryName);
  if (fs.existsSync(workspaceRelease)) {
    return workspaceRelease;
  }

  return null;
}

function ensureBinary(base) {
  const binary = resolveExistingBinary(base);
  if (binary) {
    return binary;
  }
  return null;
}

module.exports = {
  ensureBinary,
  getBinaryName,
};
