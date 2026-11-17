# nix develop .#explorer. One Cargo build directory for every worktree: the second worktree
# compiles only the crates of the workspace. A value from the environment stays.
export CARGO_BUILD_BUILD_DIR="${CARGO_BUILD_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/dcl-one/explorer-build}"
# Linux: the client loads CEF from CEF_PATH and the window system, Vulkan and audio by name.
if [[ -n ${EXPLORER_RUNTIME_LIBRARIES:-} ]]; then
  export LD_LIBRARY_PATH="${CEF_PATH:+$CEF_PATH:}$EXPLORER_RUNTIME_LIBRARIES${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
# macOS: Apple's compiler and linker. The Rust toolchain of the shell brings a clang that sets CC
# and CXX when the shell starts, so the values are set here, after it.
if [[ $(uname -s) == Darwin ]]; then
  export CC=/usr/bin/clang CXX=/usr/bin/clang++ CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang
fi
