#!/bin/bash
set -e

# Define variables
GITHUB_REPO="Goldziher/uncomment"
BINARY_NAME="uncomment"
INSTALL_DIR="/usr/local/bin"

# Print banner
echo "Installing $BINARY_NAME..."

# Determine latest version
echo "Checking for latest version..."
LATEST_VERSION=$(curl -s "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
echo "Latest version: $LATEST_VERSION"

# Determine OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map architecture names
case "$ARCH" in
    x86_64)
        ARCH="x86_64"
        ;;
    amd64)
        ARCH="x86_64"
        ;;
    arm64|aarch64)
        ARCH="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# Determine target based on OS and architecture
if [ "$OS" = "darwin" ]; then
    # macOS
    TARGET="$ARCH-apple-darwin"
    ARCHIVE_EXT="tar.gz"
elif [ "$OS" = "linux" ]; then
    # Linux
    TARGET="x86_64-unknown-linux-gnu"
    ARCHIVE_EXT="tar.gz"
elif [[ "$OS" =~ ^(msys|mingw|cygwin|windows)$ ]]; then
    # Windows
    TARGET="x86_64-pc-windows-msvc"
    ARCHIVE_EXT="zip"
    echo "Windows installation is not fully supported by this script."
    echo "Please download the binary manually from: https://github.com/$GITHUB_REPO/releases/latest"
    exit 1
else
    echo "Unsupported operating system: $OS"
    exit 1
fi

# Construct download URL
DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$LATEST_VERSION/$BINARY_NAME-$TARGET-$LATEST_VERSION.$ARCHIVE_EXT"
echo "Downloading from: $DOWNLOAD_URL"

# Create temporary directory
TMP_DIR=$(mktemp -d)
cd "$TMP_DIR"

# Download the archive
curl -L "$DOWNLOAD_URL" -o "$BINARY_NAME.$ARCHIVE_EXT"

# Extract the archive
if [ "$ARCHIVE_EXT" = "tar.gz" ]; then
    tar -xzf "$BINARY_NAME.$ARCHIVE_EXT"
else
    unzip "$BINARY_NAME.$ARCHIVE_EXT"
fi

# Make binary executable
chmod +x "$BINARY_NAME"

# Install binary
echo "Installing to $INSTALL_DIR/$BINARY_NAME..."
if [ -w "$INSTALL_DIR" ]; then
    mv "$BINARY_NAME" "$INSTALL_DIR/"
else
    # Need sudo to write to install directory
    echo "Elevated permissions required to install to $INSTALL_DIR"
    sudo mv "$BINARY_NAME" "$INSTALL_DIR/"
fi

# Clean up
cd - > /dev/null
rm -rf "$TMP_DIR"

# Verify installation
if command -v "$BINARY_NAME" > /dev/null; then
    echo "$BINARY_NAME $LATEST_VERSION has been installed successfully!"
    echo "Run '$BINARY_NAME --help' to get started."
else
    echo "Installation failed. Please check if $INSTALL_DIR is in your PATH."
    exit 1
fi
