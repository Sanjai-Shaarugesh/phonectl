#!/bin/bash

set -e

echo "📦 Building phonectl in release mode..."
cargo build --release

echo "📂 Copying binary to /usr/local/bin/..."
sudo cp target/release/phonectl /usr/local/bin/

echo "✅ Installed phonectl to /usr/local/bin/"
