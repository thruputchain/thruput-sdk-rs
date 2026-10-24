# thruput-native-exec Build Configuration

The `thruput-native-exec` crate provides Rust bindings to the hogd test_ffi library,
linked in through the `thruput-sys` crate. `thruput-sys`'s build script
obtains the library in one of three ways, selected by environment variables.

## Default Behavior

**On supported platforms** (x86_64 Linux, aarch64 Linux, aarch64 macOS), the
build script **automatically downloads** prebuilt libraries from
a public S3 bucket. No environment variables, extra features, or hogd checkout
are required.

**On unsupported platforms**, you must use the build-from-source or custom
library path described below.

## Build Methods

### 1. Download Prebuilt Library (Default on Supported Platforms)

```bash
# No environment variable needed
cargo build
```

This will automatically:
- Download the prebuilt tarball from public S3 using `curl`
- Verify the SHA256 checksum
- Assert the archive contains exactly the expected files, then extract and
  link the library
- Copy the bundled `bindings.rs` from the tarball (no hogd checkout needed)

**Supported platforms:**
- Linux x86_64: `libtest_ffi-linux-x86_64.tar.gz`
- Linux ARM64: `libtest_ffi-linux-arm64.tar.gz`
- macOS ARM64: `libtest_ffi-macos-arm64.tar.gz`

Downloads are public and require no authentication or AWS credentials.

### 2. Build from Source (Override)

Building from source needs two things: the `source-build` cargo feature
(which makes the `cmake` and `bindgen` build-dependencies available — they
are not compiled on the default path) and the `HOGD_FFI_BUILD=1` environment
variable (which selects the source build for this invocation):

```bash
# Building thruput-sys / native-exec directly:
HOGD_FFI_BUILD=1 cargo build -p thruput-sys --features source-build

# From a contract crate that uses the SDK's dev/mock features for testing,
# the feature is forwarded through the SDK:
HOGD_FFI_BUILD=1 cargo test --features thruput-sdk/source-build
```

The feature is forwarded along the dependency chain
`thruput-sdk/source-build` → `thruput-native-exec/source-build` →
`thruput-sys/source-build`. If you build from source routinely, enable
`source-build` permanently in your manifest's feature forwarding and just
toggle `HOGD_FFI_BUILD=1` per invocation — with the feature on but the
environment variable unset, you still get the prebuilt download.

This will:
- Clone hogd at the matching `test-ffi-v{VERSION}` tag (to `OUT_DIR`, or use
  `HOGD_SRC` if set), and verify the tag
- Generate bindings using bindgen from the hogd header file
- Build the library using CMake (requires CMake, a C++ toolchain, and ccache)

Setting `HOGD_FFI_BUILD=1` without the `source-build` feature fails
immediately with instructions, rather than silently falling back to the
prebuilt download.

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
- Use a prebuilt library from the specified directory, which must contain
  the appropriate library file for your platform and a matching
  `bindings.rs`
- Override the default prebuilt download behavior

## Additional Environment Variables

- `HOGD_SRC`: Path to a hogd checkout for source builds (cloned on demand
  if unset)
- `HOGD_DOWNLOAD_CACHE_DIR`: Override the prebuilt download cache location
  (default: under `OUT_DIR`)
- `NATIVE_EXEC_ALWAYS_CONFIGURE`: Force CMake reconfiguration when building
  from source

## Version Management

The version is managed differently depending on the build path:

### Prebuilt Download Path (default)

Uses the `TEST_FFI_VERSION` constant in `crates/hogd-sys/build.rs` to
determine which release to download. The prebuilt tarball includes both the
library and a pre-generated `bindings.rs`, so no hogd checkout is needed.

Current version: **0.41.0** (tag: `test-ffi-v0.41.0`)

### Source / Custom-lib-dir Path

Parses the version from `hogd/test_ffi/test_bindings.h` by reading the
following constants:
- `TEST_FFI_VERSION_MAJOR`
- `TEST_FFI_VERSION_MINOR`
- `TEST_FFI_VERSION_PATCH`

These are combined to form the version string (e.g., `0.17.0`) and used to:
1. Cross-check against `TEST_FFI_VERSION` in build.rs (must match)
2. Verify the hogd checkout is at the correct git tag (`test-ffi-v{VERSION}`)

If the version doesn't match, the build will fail with instructions to check
out the correct tag.

### Updating the Version

When updating to a new test_ffi version:
1. Update `TEST_FFI_VERSION` in `crates/hogd-sys/build.rs`
2. Update the version in this README

## Error Handling

The build script will **fail with a clear error message** if:
- Multiple build methods are specified
- The platform is unsupported and no override is given
- `HOGD_FFI_BUILD=1` is set without the `source-build` feature
- The hogd version tag doesn't match
- A download fails, checksums don't match, or the archive contains anything
  other than the expected files
- Required files are missing

This is intentional to prevent silent failures and ensure reproducible builds.

## Prebuilt Releases

Prebuilt libraries are published to:
`https://groundhog-public-release-artifacts-017820684264.s3.us-west-2.amazonaws.com/test-ffi/test-ffi-v{VERSION}/`

Each release includes, per platform:
- `libtest_ffi-<platform>.tar.gz` (+ `.sha256`)
