#!/bin/bash

set -e

BINARY_PATH="/usr/local/bin/phonectl"

if [ -f "$BINARY_PATH" ]; then
    echo "🧹 Removing phonectl from $BINARY_PATH..."
    sudo rm -f "$BINARY_PATH"
    echo "✅ phonectl has been uninstalled."
else
    echo "❌ phonectl not found at $BINARY_PATH"
fi
