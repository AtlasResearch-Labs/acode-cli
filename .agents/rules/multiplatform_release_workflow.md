# Atlas Daedalus — Multi-Platform Release & Binary Parity Workflow

## 1. Core Engineering Principle: Zero-Toolchain User Experience
- **End-user guarantee**: Developers installing `acode` via NPM (`npm install -g acode-cli` or `npx acode-cli`) on ANY platform (Windows, Linux, macOS, Android Termux) must **NEVER** be instructed or required to install Rust, Cargo, Clang, or native build toolchains.
- The product promise is instant sub-second native execution out-of-the-box via pre-compiled native binaries.
- If a user reports a version mismatch or execution issue on their device, the problem is ALWAYS binary packaging/distribution parity — NEVER an expectation for the user to compile from source.

---

## 2. Platform Binary Parity Rule (Crucial Invariant)
The NPM package distributes pre-compiled binaries in `npm/bin/`:
- `npm/bin/acode.exe` & `acode-win-x64.exe` (Windows x86_64)
- `npm/bin/acode-linux-x64` (Linux x86_64)
- `npm/bin/acode-android-arm64` (Android Bionic aarch64 for Termux)

**Invariant**:
Whenever a new version is released (e.g., `v1.8.x`), **ALL** platform binaries in `npm/bin/` must be synchronized to that exact version before publishing to NPM. Never publish an NPM version where Windows has been updated from a local build while Android or Linux binaries remain frozen on older legacy versions.

---

## 3. Cross-Platform Compilation Pipeline
1. **Local Host Constraints**:
   - A local Windows developer workstation only natively builds Windows binaries (`acode.exe`).
   - Android Bionic (`aarch64-linux-android`) requires Android NDK toolchains (`aarch64-linux-android24-clang`, `llvm-ar`).
2. **Cloud CI / Multi-Platform Build**:
   - Workflows in `.github/workflows/` (Ubuntu runners) use `actions/checkout`, `dtolnay/rust-toolchain`, and `nttld/setup-ndk` to cross-compile Android and Linux binaries.
   - Multi-platform release binaries must be assembled into `npm/bin/` through automated CI before `npm publish`.

---

## 4. Termux (Android) Environment Dynamics
- Global installation command: `npm install -g acode-cli@latest`
- Binary link destination: `$PREFIX/bin/acode`
- Launcher behavior: `npm/bin/acode` detects Android environment (`$PREFIX` or `/system/bin/sh`) and ARM64 architecture, and executes `$BASEDIR/acode-android-arm64`.
- If an older binary is shadowing in `$PATH` (e.g. from an old test install), inspect with `which acode` and reset shell command hashing with `hash -r`.
