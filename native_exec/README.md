# native_exec Build Configuration

The `native_exec` crate provides Rust bindings to the hogd test_ffi library.

## Default Behavior

**On supported platforms** (x86_64 Linux, aarch64 macOS), the build script will **automatically download** prebuilt libraries from GitHub releases. No environment variables are required, and **the hogd submodule is NOT needed**.

**On unsupported platforms**, you must set either `HOGD_FFI_BUILD=1` or `HOGD_FFI_LIB_DIR` (both require the hogd submodule).

## Build Methods

### 1. Download Prebuilt Library (Default on Supported Platforms)

```bash
# No environment variable needed on x86_64-linux or aarch64-macos
cargo build
```

This will automatically:
- Download the prebuilt library from GitHub releases for your platform
- Verify the SHA256 checksum
- Extract and link the library
- Copy the bundled `bindings.rs` from the tarball (no hogd submodule needed)

**Supported platforms:**
- Linux x86_64: `libtest_ffi-linux-x86_64.tar.gz`
- macOS ARM64: `libtest_ffi-macos-arm64.tar.gz`

**⚠️ Important: GitHub CLI Requirement (Temporary)**

Currently, downloads use the `gh` CLI tool because the groundhogchain/hogd repository is **private**. You must have:
- GitHub CLI (`gh`) installed
- Authenticated with `gh auth login`
- Access to the groundhogchain/hogd repository

Once the repository becomes **public**, the build script will be updated to use direct HTTP downloads (via `reqwest`), which won't require the `gh` CLI or authentication. The commented-out HTTP download code is already in `build.rs` for this future transition.

### 2. Build from Source (Override)

```bash
export HOGD_FFI_BUILD=1
cargo build
```

This will:
- Build the library using CMake (requires CMake, C++, and ccache)
- Generate bindings using bindgen from the hogd header file
- Verify that the hogd submodule is checked out to the correct tag
- Override the default prebuilt download behavior

**Requires:** hogd submodule checked out (`git submodule update --init --recursive`)

**Use this when:**
- You're on an unsupported platform
- You want to build with custom flags or modifications
- You're developing changes to the C++ code

### 3. Use Custom Library Path (Override)

```bash
export HOGD_FFI_LIB_DIR=/path/to/lib
cargo build
```

This will:
- Use a prebuilt library from the specified directory
- Generate bindings using bindgen from the hogd header file
- The directory must contain the appropriate library file for your platform
- Override the default prebuilt download behavior

**Requires:** hogd submodule checked out (for generating bindings)

## Additional Environment Variables

- `HOGD_SRC`: Override the path to the hogd source (default: `../hogd/`)
- `NATIVE_EXEC_ALWAYS_CONFIGURE`: Force CMake reconfiguration when building from source

## Version Management

The version is managed differently depending on the build path:

### Prebuilt Download Path (default)

Uses the `TEST_FFI_VERSION` constant in `native_exec/build.rs` to determine which release to download. The prebuilt tarball includes both the library and a pre-generated `bindings.rs`, so no hogd submodule is needed.

Current version: **0.3.4** (tag: `test-ffi-v0.3.4`)

### Source / Custom-lib-dir Path

Parses the version from `hogd/test_ffi/test_bindings.h` by reading the following constants:
- `TEST_FFI_VERSION_MAJOR`
- `TEST_FFI_VERSION_MINOR`
- `TEST_FFI_VERSION_PATCH`

These are combined to form the version string (e.g., `0.3.4`) and used to:
1. Cross-check against `TEST_FFI_VERSION` in build.rs (must match)
2. Verify the hogd submodule is at the correct git tag (`test-ffi-v{VERSION}`)

If the version doesn't match, the build will fail with instructions to check out the correct tag:

```bash
cd ../hogd && git checkout test-ffi-v0.3.4
```

### Updating the Version

When updating to a new test_ffi version:
1. Update `TEST_FFI_VERSION` in `native_exec/build.rs`
2. Update the version in this README
3. Ensure the hogd submodule is at the matching tag (for source builds)

## Error Handling

The build script will **fail with a clear error message** if:
- No build method is specified
- Multiple build methods are specified
- The hogd version tag doesn't match
- A download fails or checksums don't match
- Required files are missing

This is intentional to prevent silent failures and ensure reproducible builds.

## GitHub Releases

Prebuilt libraries are published to: https://github.com/groundhogchain/hogd/releases/tag/test-ffi-v{VERSION}

Each release includes:
- `libtest_ffi-linux-x86_64.tar.gz` (+ `.sha256`)
- `libtest_ffi-macos-arm64.tar.gz` (+ `.sha256`)

**Repository Status**: Currently **private** (requires `gh` CLI with authentication). Will be made **public** in the future, at which point direct HTTP downloads will be enabled.
