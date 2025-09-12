#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -eq 0 ]]; then
        print_error "This script should not be run as root"
        exit 1
    fi
}

# Check system requirements
check_requirements() {
    print_status "Checking system requirements..."
    
    # Check if git is installed
    if ! command -v git &> /dev/null; then
        print_error "git is not installed. Please install git first."
        print_status "Ubuntu/Debian: sudo apt install git"
        print_status "Fedora: sudo dnf install git"
        print_status "Arch: sudo pacman -S git"
        exit 1
    fi
    
    # Check if cargo/rust is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Rust/Cargo is not installed. Please install Rust first."
        print_status "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        print_status "Then restart your terminal or run: source ~/.cargo/env"
        exit 1
    fi
    
    # Check if adb is available (optional warning)
    if ! command -v adb &> /dev/null; then
        print_warning "ADB is not installed. You'll need it to use phonectl."
        print_status "Ubuntu/Debian: sudo apt install android-tools-adb"
        print_status "Fedora: sudo dnf install android-tools"
        print_status "Arch: sudo pacman -S android-tools"
    fi
    
    print_success "Requirements check passed"
}

# Install phonectl
install_phonectl() {
    print_status "Starting phonectl installation..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    print_status "Using temporary directory: $TEMP_DIR"
    
    # Clone the repository
    print_status "Cloning phonectl repository..."
    git clone https://github.com/Sanjai-Shaarugesh/phonectl.git "$TEMP_DIR/phonectl"
    
    # Change to the project directory
    cd "$TEMP_DIR/phonectl"
    
    # Build the project
    print_status "Building phonectl in release mode (this may take a few minutes)..."
    cargo build --release
    
    # Determine installation path
    if [[ ":$PATH:" == *":$HOME/.local/bin:"* ]] || [[ -d "$HOME/.local/bin" ]]; then
        # Install to user's local bin if it exists or is in PATH
        INSTALL_PATH="$HOME/.local/bin"
        print_status "Installing phonectl to $INSTALL_PATH..."
        mkdir -p "$INSTALL_PATH"
        cp target/release/phonectl "$INSTALL_PATH/"
        chmod +x "$INSTALL_PATH/phonectl"
        NEEDS_PATH_UPDATE=false
        
        # Check if ~/.local/bin is in PATH
        if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
            NEEDS_PATH_UPDATE=true
        fi
    else
        # Install to system-wide location
        INSTALL_PATH="/usr/local/bin"
        print_status "Installing phonectl to $INSTALL_PATH (requires sudo)..."
        sudo cp target/release/phonectl "$INSTALL_PATH/"
        sudo chmod +x "$INSTALL_PATH/phonectl"
        NEEDS_PATH_UPDATE=false
    fi
    
    # Clean up
    cd /
    rm -rf "$TEMP_DIR"
    
    print_success "phonectl installed successfully to $INSTALL_PATH!"
    
    # PATH update notification if needed
    if [[ "$NEEDS_PATH_UPDATE" == "true" ]]; then
        print_warning "Note: ~/.local/bin is not in your PATH"
        print_status "Add this line to your ~/.bashrc or ~/.zshrc:"
        echo "export PATH=\"\$HOME/.local/bin:\$PATH\""
        print_status "Then restart your terminal or run: source ~/.bashrc"
    fi
}

# Verify installation
verify_installation() {
    print_status "Verifying installation..."
    
    # Update hash cache
    hash -r 2>/dev/null || true
    
    if command -v phonectl &> /dev/null; then
        VERSION=$(phonectl --version 2>/dev/null || echo "installed")
        print_success "phonectl is installed and accessible"
        print_status "Location: $(which phonectl)"
        print_status "Version: $VERSION"
    else
        print_error "Installation verification failed"
        print_status "phonectl is not in your PATH"
        print_status "Try restarting your terminal or running: hash -r"
        return 1
    fi
}

# Show usage information
show_usage() {
    echo
    print_success "🎉 Installation complete! Here's how to get started:"
    echo
    echo "1. Setup ADB connection:"
    echo "   phonectl setup"
    echo
    echo "2. Basic commands:"
    echo "   phonectl unlock           - Unlock phone and keep awake"
    echo "   phonectl list             - List all contacts"
    echo "   phonectl search <query>   - Search contacts"
    echo "   phonectl call <name|number> - Make a call"
    echo "   phonectl answer           - Answer incoming call"
    echo "   phonectl reject           - Reject/end call"
    echo
    echo "📚 For more information:"
    echo "   https://github.com/Sanjai-Shaarugesh/phonectl"
    echo
}

# Main installation process
main() {
    echo "📱 phonectl Installation Script"
    echo "================================"
    echo
    
    check_root
    check_requirements
    install_phonectl
    
    if verify_installation; then
        show_usage
    else
        print_error "Installation completed but verification failed"
        print_status "You may need to restart your terminal"
        exit 1
    fi
}

# Run main function
main "$@"
