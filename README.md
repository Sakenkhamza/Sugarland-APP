# Sugarland

Sugarland — Liquidation Management Platform.

## Development Setup

### Windows

1. Install [Rust](https://rustup.rs/) and [Node.js](https://nodejs.org/).
2. Run `npm install` to install frontend dependencies.
3. Run `start_dev.bat` to launch the development environment.
4. Run `build.bat` to build the production release.

### macOS

1. Install Xcode Command Line Tools:
   ```bash
   xcode-select --install
   ```
2. Install Rust:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. Install Node.js (via [nvm](https://github.com/nvm-sh/nvm) or [Homebrew](https://brew.sh/)):
   ```bash
   brew install node
   ```
4. Install frontend dependencies:
   ```bash
   npm install
   ```
5. Launch the application in development mode:
   ```bash
   ./start_dev.sh
   ```
6. Build the application for production:
   ```bash
   ./build.sh
   ```
