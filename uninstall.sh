#!/bin/bash

# Universal phonectl uninstaller
# Works on all computers regardless of installation location

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to remove phonectl from a specific path
remove_from_path() {
    local path="$1"
    local needs_sudo="$2"
    
    if [ -f "$path" ]; then
        print_status "Removing phonectl from $path..."
        
        if [ "$needs_sudo" = "true" ]; then
            if sudo rm -f "$path"; then
                print_success "Successfully removed $path"
                return 0
            else
                print_error "Failed to remove $path"
                return 1
            fi
        else
            if rm -f "$path"; then
                print_success "Successfully removed $path"
                return 0
            else
                print_error "Failed to remove $path"
                return 1
            fi
        fi
    fi
    return 1
}

# Main uninstallation logic
main() {
    echo "🧹 Starting phonectl uninstallation..."
    local removed=false
    local found_locations=()
    
    # Check all possible installation locations
    # User-specific locations (no sudo needed)
    local user_locations=(
        "$HOME/.local/bin/phonectl"
        "$HOME/bin/phonectl"
        "$HOME/.cargo/bin/phonectl"
    )
    
    # System-wide locations (sudo needed)
    local system_locations=(
        "/usr/local/bin/phonectl"
        "/usr/bin/phonectl"
        "/bin/phonectl"
        "/opt/phonectl/bin/phonectl"
    )
    
    # Check user locations first
    for location in "${user_locations[@]}"; do
        if [ -f "$location" ]; then
            found_locations+=("$location")
            if remove_from_path "$location" "false"; then
                removed=true
            fi
        fi
    done
    
    # Check system locations
    for location in "${system_locations[@]}"; do
        if [ -f "$location" ]; then
            found_locations+=("$location")
            if remove_from_path "$location" "true"; then
                removed=true
            fi
        fi
    done
    
    # Try to find phonectl using 'which' command
    if command -v phonectl &> /dev/null; then
        local which_location=$(which phonectl)
        print_status "Found phonectl in PATH at: $which_location"
        
        # Check if this location is already in our found_locations
        local already_found=false
        for loc in "${found_locations[@]}"; do
            if [ "$loc" = "$which_location" ]; then
                already_found=true
                break
            fi
        done
        
        # If not already processed, try to remove it
        if [ "$already_found" = "false" ]; then
            # Determine if we need sudo based on the path
            if [[ "$which_location" == "/usr/"* ]] || [[ "$which_location" == "/bin/"* ]] || [[ "$which_location" == "/opt/"* ]]; then
                if remove_from_path "$which_location" "true"; then
                    removed=true
                fi
            else
                if remove_from_path "$which_location" "false"; then
                    removed=true
                fi
            fi
        fi
    fi
    
    # Try cargo uninstall if cargo is available
    if command -v cargo &> /dev/null; then
        if cargo install --list 2>/dev/null | grep -q "^phonectl "; then
            print_status "Found phonectl installed via cargo, removing..."
            if cargo uninstall phonectl &> /dev/null; then
                print_success "Successfully removed phonectl via cargo"
                removed=true
            else
                print_error "Failed to remove phonectl via cargo"
            fi
        fi
    fi
    
    # Clear shell hash cache
    hash -r 2>/dev/null || true
    
    # Final verification
    if command -v phonectl &> /dev/null; then
        print_error "phonectl is still accessible after uninstallation"
        print_status "Location: $(which phonectl)"
        print_status "You may need to manually remove it or restart your terminal"
        exit 1
    else
        if [ "$removed" = "true" ]; then
            print_success "✅ phonectl has been successfully uninstalled!"
        else
            print_error "❌ phonectl not found in any expected location"
            exit 1
        fi
    fi
}

# Run main function
main "$@"
