#!/usr/bin/env bash
# Source from a script whose working directory is the repository root.
if [ -x "$PWD/.tools/cargo/bin/cargo" ]; then
  export RUSTUP_HOME="$PWD/.tools/rustup"
  export CARGO_HOME="$PWD/.tools/cargo"
  export PATH="$PWD/.tools/cargo/bin:$PATH"
fi
export NEXT_TELEMETRY_DISABLED=1
