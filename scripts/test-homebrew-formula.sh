#!/bin/bash
# Script to test Homebrew formula locally

set -e

echo "Testing Homebrew formula locally..."

# Add the tap if not already added
if ! brew tap | grep -q "goldziher/tap"; then
    echo "Adding local tap..."
    brew tap goldziher/tap $(pwd)/homebrew-tap
else
    echo "Tap already exists, updating..."
    brew untap goldziher/tap
    brew tap goldziher/tap $(pwd)/homebrew-tap
fi

# Install from the local tap
echo "Installing uncomment from local tap..."
brew install --verbose goldziher/tap/uncomment

# Test the installation
echo "Testing uncomment command..."
uncomment --version

# Create a test file
cat > /tmp/test_uncomment.py << 'EOF'
# This is a comment
def hello():
    print("Hello, world!")  # Inline comment
    # Another comment
    return True
EOF

echo "Running uncomment on test file..."
uncomment --dry-run /tmp/test_uncomment.py

echo "✅ Homebrew formula test completed successfully!"

# Cleanup
echo "Cleaning up..."
brew uninstall uncomment
brew untap goldziher/tap
