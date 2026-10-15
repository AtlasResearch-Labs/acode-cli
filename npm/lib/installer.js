const fs = require('fs');
const path = require('path');
const https = require('https');

function getPlatformKey() {
  const isAndroid = process.platform === 'android';
  const platform = process.platform;
  let arch = process.arch;
  if (arch === 'arm' || arch === 'arm64' || arch === 'aarch64') {
    arch = 'arm64';
  } else if (arch === 'x64' || arch === 'x86_64') {
    arch = 'x64';
  }
  return { platform, arch, isAndroid, key: `${platform}-${arch}` };
}

function resolveExistingBinary(base) {
  const { platform, arch, isAndroid, key } = getPlatformKey();
  const isWin = platform === 'win32';

  const candidates = [];

  if (isAndroid) {
    // Android Bionic native targets (Termux)
    candidates.push(
      path.join(__dirname, '..', 'bin', 'native', `${base}-android-${arch}`),
      path.join(__dirname, '..', 'bin', 'native', `${base}-android-arm64`),
      path.join(__dirname, '..', 'bin', 'native', `${base}-linux-${arch}`)
    );
  } else {
    // Desktop POSIX and Windows targets
    candidates.push(
      path.join(__dirname, '..', 'bin', 'native', `${base}-${platform}-${arch}${isWin ? '.exe' : ''}`),
      path.join(__dirname, '..', 'bin', 'native', `${base}-${key}${isWin ? '.exe' : ''}`)
    );
  }

  candidates.push(
    // Generic binary name in bin/native/
    path.join(__dirname, '..', 'bin', 'native', isWin ? `${base}.exe` : base),
    // User global daedalus binary cache ~/.daedalus/bin/
    path.join(process.env.HOME || process.env.USERPROFILE || '', '.daedalus', 'bin', isWin ? `${base}.exe` : base),
    path.join(process.env.HOME || process.env.USERPROFILE || '', '.daedalus', 'bin', `${base}-${key}${isWin ? '.exe' : ''}`),
    // System cargo bin ~/.cargo/bin/
    path.join(process.env.HOME || process.env.USERPROFILE || '', '.cargo', 'bin', isWin ? `${base}.exe` : base),
    // Local workspace development target/release
    path.resolve(__dirname, '..', '..', 'target', 'release', isWin ? `${base}.exe` : base),
    path.resolve(__dirname, '..', '..', 'target', `${arch === 'arm64' ? 'aarch64' : 'x86_64'}-linux-android`, 'release', base),
    path.resolve(__dirname, '..', '..', 'target', `${arch === 'arm64' ? 'aarch64' : 'x86_64'}-unknown-linux-gnu`, 'release', base)
  );

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return null;
}

function downloadBinary(base, targetPath) {
  return new Promise((resolve, reject) => {
    const { platform, arch, key } = getPlatformKey();
    const isWin = platform === 'win32';
    const remoteFilename = isWin ? `${base}-windows-x64.exe` : `${base}-${key}`;
    const pkg = require('../package.json');
    const url = `https://github.com/AtlasResearch-Labs/acode-cli/releases/download/v${pkg.version}/${remoteFilename}`;

    console.log(`[ACODE] Downloading native engine for ${key} from GitHub releases...`);

    const dir = path.dirname(targetPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }

    function fetchWithRedirects(fetchUrl, redirectsLeft = 5) {
      if (redirectsLeft === 0) {
        return reject(new Error('Too many redirects while downloading native binary.'));
      }

      https.get(fetchUrl, { headers: { 'User-Agent': 'acode-cli-installer' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return fetchWithRedirects(res.headers.location, redirectsLeft - 1);
        }

        if (res.statusCode !== 200) {
          return reject(new Error(`Failed to download binary (${fetchUrl}): HTTP ${res.statusCode}`));
        }

        const fileStream = fs.createWriteStream(targetPath);
        res.pipe(fileStream);

        fileStream.on('finish', () => {
          fileStream.close(() => {
            if (!isWin) {
              try {
                fs.chmodSync(targetPath, 0o755);
              } catch {}
            }
            console.log('[ACODE] Native engine installed successfully.');
            resolve(targetPath);
          });
        });

        fileStream.on('error', (err) => {
          fs.unlink(targetPath, () => {});
          reject(err);
        });
      }).on('error', reject);
    }

    fetchWithRedirects(url);
  });
}

async function ensureBinary(base) {
  const binary = resolveExistingBinary(base);
  if (binary) {
    return binary;
  }

  // Attempt auto-download to ~/.daedalus/bin/
  const home = process.env.HOME || process.env.USERPROFILE || '';
  if (home) {
    const isWin = process.platform === 'win32';
    const targetName = isWin ? `${base}.exe` : base;
    const targetPath = path.join(home, '.daedalus', 'bin', targetName);
    try {
      return await downloadBinary(base, targetPath);
    } catch (err) {
      console.warn(`[ACODE] Auto-download failed: ${err.message}`);
    }
  }

  return null;
}

module.exports = {
  ensureBinary,
  resolveExistingBinary,
  getPlatformKey,
  downloadBinary
};
