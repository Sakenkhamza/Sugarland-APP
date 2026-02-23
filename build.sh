#!/bin/bash
export PATH="$HOME/.cargo/bin:$PATH"
cd src-tauri
cargo clean
RUST_BACKTRACE=1 cargo build --release
cd ..
npm run tauri build
