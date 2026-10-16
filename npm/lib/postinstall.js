#!/usr/bin/env node
/**
 * Atlas Daedalus — Postinstall Native Engine Linker
 * Detects host platform and sets up a zero-overhead native launcher trampoline
 * completely removing Node runtime from the execution path.
 * (C) 2026 Atlas Research Labs — MIT License
 */

const fs = require('fs');
const path = require('path');
const { ensureBinary, getPlatformKey } = require('./installer');

(async () => {
  try {
    const { platform, arch, isAndroid } = getPlatformKey();
    const isWin = platform === 'win32';
    const binDir = path.join(__dirname, '..', 'bin');

    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    // Resolve native engine binary
    const binaryPath = await ensureBinary('acode');

    if (!binaryPath || !fs.existsSync(binaryPath)) {
      // Fallback: leave JS wrapper in place if binary not yet resolved
      return;
    }

    if (!isWin) {
      // On POSIX (Linux, macOS, Android), write a direct shell exec trampoline:
      // exec replaces the current process image in <1ms without starting Node!
      // Startup latency: ~2ms | RSS: ~8MB (pure native Rust)
      const trampolineScript = `#!/bin/sh
# Atlas Daedalus Native Direct Launcher (Pure Rust Engine)
# Bypasses Node.js V8 runtime for instant native execution (~2ms startup, 8MB RSS)
TARGET_BIN="${binaryPath}"

if [ -x "$TARGET_BIN" ]; then
  exec "$TARGET_BIN" "$@"
fi

# Fallback to Node wrapper if native binary missing or unexecutable
BASEDIR=$(dirname "$0")
exec node "$BASEDIR/acode.js" "$@"
`;

      const trampolinePath = path.join(binDir, 'acode');
      const daedalusTrampoline = path.join(binDir, 'daedalus');

      fs.writeFileSync(trampolinePath, trampolineScript, { mode: 0o755 });
      fs.writeFileSync(daedalusTrampoline, trampolineScript, { mode: 0o755 });

      try {
        fs.chmodSync(binaryPath, 0o755);
      } catch (_) {}

      console.log(`[ACODE] Configured native direct launcher for ${platform}-${arch} (Node excluded from run path).`);
    } else {
      // On Windows, create a direct .cmd trampoline invoking the native executable directly
      const cmdScript = `@echo off
rem Atlas Daedalus Native Direct Launcher (Pure Rust Engine)
set "TARGET_BIN=${binaryPath}"
if exist "%TARGET_BIN%" (
  "%TARGET_BIN%" %*
  exit /b %ERRORLEVEL%
)
node "%~dp0acode.js" %*
`;

      const cmdPath = path.join(binDir, 'acode.cmd');
      const daedalusCmd = path.join(binDir, 'daedalus.cmd');

      fs.writeFileSync(cmdPath, cmdScript, 'utf8');
      fs.writeFileSync(daedalusCmd, cmdScript, 'utf8');

      // Also copy or link native binary as acode.exe in bin/ if appropriate
      const destExe = path.join(binDir, 'acode.exe');
      if (path.resolve(binaryPath) !== path.resolve(destExe)) {
        try {
          fs.copyFileSync(binaryPath, destExe);
        } catch (_) {}
      }

      console.log(`[ACODE] Configured native direct launcher for Windows x64 (Node excluded from run path).`);
    }
  } catch (err) {
    // Non-fatal: JS wrapper fallback in bin/acode.js handles execution gracefully
  }
})();
