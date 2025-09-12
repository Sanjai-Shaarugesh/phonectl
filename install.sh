#!/bin/bash

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

echo "📱 phonectl Installation Script"
echo "================================"
echo

# Check requirements
print_status "Checking requirements..."

if ! command -v git &> /dev/null; then
    print_error "git is required but not installed"
    print_status "Install with: sudo apt install git (Ubuntu/Debian) or sudo dnf install git (Fedora)"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    print_error "Rust/Cargo is required but not installed"
    print_status "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    print_status "Then run: source ~/.cargo/env"
    exit 1
fi

print_success "Requirements check passed"

# Create temporary directory and clone
TEMP_DIR=$(mktemp -d)
print_status "Downloading phonectl source to $TEMP_DIR..."

cd "$TEMP_DIR"
git clone https://github.com/Sanjai-Shaarugesh/phonectl.git

# Enter the project directory
cd phonectl

# Build the project
print_status "Building phonectl in release mode..."
cargo build --release

# Install based on available paths
if [[ -d "$HOME/.local/bin" ]] || mkdir -p "$HOME/.local/bin" 2>/dev/null; then
    INSTALL_PATH="$HOME/.local/bin"
    print_status "Installing to $INSTALL_PATH..."
    cp target/release/phonectl "$INSTALL_PATH/"
    chmod +x "$INSTALL_PATH/phonectl"
    
    # Check if ~/.local/bin is in PATH
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        print_warning "~/.local/bin is not in your PATH"
        echo "Add this to your ~/.bashrc or ~/.zshrc:"
        echo "export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo "Then run: source ~/.bashrc (or restart terminal)"
    fi
else
    INSTALL_PATH="/usr/local/bin"
    print_status "Installing to $INSTALL_PATH (requires sudo)..."
    sudo cp target/release/phonectl "$INSTALL_PATH/"
    sudo chmod +x "$INSTALL_PATH/phonectl"
fi

# Cleanup
cd /
rm -rf "$TEMP_DIR"

# Verify installation
hash -r 2>/dev/null || true

if command -v phonectl &> /dev/null; then
    print_success "✅ phonectl installed successfully!"
    print_status "Location: $(which phonectl)"
    print_status "Version: $(phonectl --version 2>/dev/null || echo 'installed')"
    echo
    echo "🚀 Quick start:"
    echo "  phonectl setup    # Setup ADB connection"
    echo "  phonectl list     # List contacts"
    echo "  phonectl call <name|number>  # Make a call"
else
    print_error "Installation failed - phonectl not found in PATH"
    print_status "Try restarting your terminal or run: hash -r"
    exit 1
fi
