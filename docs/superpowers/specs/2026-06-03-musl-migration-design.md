# musl Static Linking Migration

## Context

rssume currently builds for `x86_64-unknown-linux-gnu` (glibc), which means the binary depends on the system's glibc version. Migrating to musl produces fully static binaries that run on any Linux distribution without runtime dependencies — ideal for containerized deployment and simplified distribution.

## Dependency Analysis

The project is almost pure Rust. The only C/assembly dependency is `ring` (v0.17.14), pulled in transitively via `reqwest → hyper-rustls → rustls → ring`. `ring` compiles its own C code using the `cc` crate and does not require system-installed C libraries.

- **No OpenSSL** — reqwest uses `rustls-tls` with `default-features = false`
- **No native-tls** — explicitly avoided
- **No system C libraries** — CI installs nothing via apt/brew

This makes the musl migration straightforward: only the toolchain needs to change.

## Design

### CI Test Job (ci.yml)

Change Linux test target:

```yaml
matrix:
  target:
    - os: ubuntu-latest
      triple: x86_64-unknown-linux-musl
      profile: release
    - os: macos-latest
      triple: aarch64-apple-darwin
      profile: release
    - os: windows-latest
      triple: x86_64-pc-windows-msvc
      profile: debug
```

Add musl toolchain installation step (conditional on Linux):

```yaml
- name: Install musl toolchain
  if: contains(matrix.target.triple, 'linux')
  run: |
    if [ "${{ matrix.target.triple }}" = "x86_64-unknown-linux-musl" ]; then
      sudo apt-get update && sudo apt-get install -y musl-tools
    else
      curl -sSL https://github.com/musl-cross/musl-cross-make/releases/download/v0.9.9/musl-cross-make-x86_64-linux-musl.tar.xz | tar -xJ -C /tmp
      echo "/tmp/cross/bin" >> $GITHUB_PATH
    fi
```

Set environment variables for aarch64 cross-compilation:

```yaml
- name: Set cross-compile env
  if: matrix.target.triple == 'aarch64-unknown-linux-musl'
  run: |
    echo "CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc" >> $GITHUB_ENV
    echo "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc" >> $GITHUB_ENV
```

**Note**: aarch64-musl targets only run `cargo build`, not `cargo test` (cannot execute aarch64 binaries on x86_64 runner).

### CI Build Job (ci.yml)

Same changes as test job, plus add aarch64-musl to the build matrix:

```yaml
matrix:
  target:
    - os: ubuntu-latest
      triple: x86_64-unknown-linux-musl
      profile: release
      ext: ""
    - os: ubuntu-latest
      triple: aarch64-unknown-linux-musl
      profile: release
      ext: ""
    - os: macos-latest
      triple: x86_64-apple-darwin
      profile: release
      ext: ""
    - os: macos-latest
      triple: aarch64-apple-darwin
      profile: release
      ext: ""
    - os: windows-latest
      triple: x86_64-pc-windows-msvc
      profile: debug
      ext: ".exe"
```

### Release Workflow (release.yml)

Same musl toolchain changes. Update Linux targets:

```yaml
matrix:
  target:
    - os: ubuntu-latest
      triple: x86_64-unknown-linux-musl
    - os: ubuntu-latest
      triple: aarch64-unknown-linux-musl
    - os: macos-latest
      triple: x86_64-apple-darwin
    - os: macos-latest
      triple: aarch64-apple-darwin
    - os: windows-latest
      triple: x86_64-pc-windows-msvc
```

### Documentation (CLAUDE.md)

Update build targets and CI section to reflect musl migration.

## Key Technical Decisions

1. **musl-cross-make for aarch64**: Ubuntu repos don't ship musl cross-compilers for aarch64. Use pre-built toolchain from `musl-cross-make` GitHub releases.
2. **x86_64 uses apt**: `musl-tools` package provides `musl-gcc` wrapper, simplest approach.
3. **aarch64 test skipped**: Cannot execute aarch64 binaries on x86_64 runner without QEMU. Build-only verification is sufficient.
4. **ring compatibility**: `ring` supports musl targets. Cross-compilation works via `CC_*` environment variables.

## Verification

1. Push changes and verify CI passes: `curl -s "https://api.github.com/repos/zyxisme/rssume/commits/<sha>/check-runs"`
2. Verify binary is statically linked: `file target/x86_64-unknown-linux-musl/release/rssume` should show "statically linked"
3. Verify release artifacts include both musl targets
