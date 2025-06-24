import os
import platform
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from urllib.request import urlopen, Request
from urllib.error import URLError


def get_platform():
    """Determine the platform and architecture for binary selection."""
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "windows":
        if machine in ["amd64", "x86_64"]:
            return "x86_64-pc-windows-msvc"
        elif machine in ["x86", "i386", "i686"]:
            return "i686-pc-windows-msvc"
    elif system == "linux":
        if machine in ["amd64", "x86_64"]:
            return "x86_64-unknown-linux-gnu"
        elif machine in ["aarch64", "arm64"]:
            return "aarch64-unknown-linux-gnu"
    elif system == "darwin":
        if machine in ["amd64", "x86_64"]:
            return "x86_64-apple-darwin"
        elif machine in ["aarch64", "arm64"]:
            return "aarch64-apple-darwin"

    raise RuntimeError(f"Unsupported platform: {system} {machine}")


def get_binary_url(version):
    """Get the download URL for the binary."""
    platform_name = get_platform()
    ext = ".exe" if platform.system().lower() == "windows" else ""
    return f"https://github.com/Goldziher/uncomment/releases/download/v{version}/uncomment-{platform_name}{ext}"


def download_binary(url, dest_path):
    """Download the binary from the given URL."""
    try:
        req = Request(url, headers={'User-Agent': 'uncomment-python-wrapper'})
        with urlopen(req) as response:
            with open(dest_path, 'wb') as f:
                f.write(response.read())
    except URLError as e:
        raise RuntimeError(f"Failed to download binary from {url}: {e}")


def get_binary_path():
    """Get the path where the binary should be stored."""
    cache_dir = Path.home() / ".cache" / "uncomment"
    cache_dir.mkdir(parents=True, exist_ok=True)

    platform_name = get_platform()
    ext = ".exe" if platform.system().lower() == "windows" else ""
    return cache_dir / f"uncomment-{platform_name}{ext}"


def ensure_binary():
    """Ensure the binary is available, downloading if necessary."""
    from . import __version__

    binary_path = get_binary_path()

    # Check if binary exists and is executable
    if binary_path.exists():
        if os.access(binary_path, os.X_OK):
            return str(binary_path)

    # Download the binary
    print(f"Downloading uncomment binary v{__version__}...", file=sys.stderr)
    url = get_binary_url(__version__)

    try:
        download_binary(url, binary_path)
        os.chmod(binary_path, 0o755)  # Make executable
        print("Binary downloaded successfully!", file=sys.stderr)
        return str(binary_path)
    except Exception as e:
        raise RuntimeError(f"Failed to setup uncomment binary: {e}")


def run_uncomment(args):
    """Run the uncomment binary with the given arguments."""
    binary_path = ensure_binary()

    try:
        # Run the binary and forward all output
        result = subprocess.run([binary_path] + args, check=False)
        sys.exit(result.returncode)
    except FileNotFoundError:
        raise RuntimeError(f"Binary not found at {binary_path}")
    except Exception as e:
        raise RuntimeError(f"Failed to run uncomment: {e}")
