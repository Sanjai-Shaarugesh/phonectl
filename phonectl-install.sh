#!/bin/bash

# phonectl Installation Script
# Usage: curl -fsSL https://raw.githubusercontent.com/Sanjai-Shaarugesh/phonectl/main/install.sh | bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
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
        exit 1
    fi
    
    # Check if cargo/rust is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Rust/Cargo is not installed. Please install Rust first."
        print_status "Visit https://rustup.rs/ to install Rust"
        exit 1
    fi
    
    # Check if adb is available (optional warning)
    if ! command -v adb &> /dev/null; then
        print_warning "ADB is not installed. You'll need it to use phonectl."
        print_status "Install ADB with: sudo apt install android-tools-adb (Ubuntu/Debian)"
        print_status "Or: sudo dnf install android-tools (Fedora)"
        print_status "Or: sudo pacman -S android-tools (Arch)"
    fi
    
    print_success "Requirements check completed"
}

# Install phonectl
install_phonectl() {
    print_status "Starting phonectl installation..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Clone the repository
    print_status "Cloning phonectl repository..."
    git clone https://github.com/Sanjai-Shaarugesh/phonectl.git
    cd phonectl
    
    # Build the project
    print_status "Building phonectl (this may take a few minutes)..."
    cargo build --release
    
    # Install to /usr/local/bin (requires sudo)
    print_status "Installing phonectl to /usr/local/bin..."
    sudo cp target/release/phonectl /usr/local/bin/
    sudo chmod +x /usr/local/bin/phonectl
    
    # Clean up
    cd /
    rm -rf "$TEMP_DIR"
    
    print_success "phonectl installed successfully!"
}

# Verify installation
verify_installation() {
    print_status "Verifying installation..."
    
    if command -v phonectl &> /dev/null; then
        VERSION=$(phonectl --version 2>/dev/null || echo "version check failed")
        print_success "phonectl is installed and accessible"
        print_status "Version: $VERSION"
    else
        print_error "Installation verification failed"
        print_status "phonectl is not in your PATH"
        exit 1
    fi
}

# Show usage information
show_usage() {
    echo
    print_success "Installation complete! Here's how to get started:"
    echo
    echo "1. Setup ADB connection:"
    echo "   phonectl setup"
    echo
    echo "2. Basic commands:"
    echo "   phonectl unlock     - Unlock phone and keep awake"
    echo "   phonectl list       - List all contacts"
    echo "   phonectl search <query> - Search contacts"
    echo "   phonectl call <name|number> - Make a call"
    echo "   phonectl answer     - Answer incoming call"
    echo "   phonectl reject     - Reject/end call"
    echo
    echo "For more information, visit: https://github.com/Sanjai-Shaarugesh/phonectl"
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
    verify_installation
    show_usage
}

# Run main function
main "$@"
