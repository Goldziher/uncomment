import os
import platform
import subprocess
import sys
import tempfile
import tarfile
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


def convert_version_to_git_tag(version):
    """Convert Python version format to git tag format."""
    # Convert 2.1.1rc1 to 2.1.1-rc.1
    if "rc" in version:
        parts = version.split("rc")
        return f"{parts[0]}-rc.{parts[1]}"
    return version


def get_binary_url(version):
    """Get the download URL for the binary."""
    platform_name = get_platform()
    git_tag_version = convert_version_to_git_tag(version)
    return f"https://github.com/Goldziher/uncomment/releases/download/v{git_tag_version}/uncomment-{platform_name}.tar.gz"


def download_binary(url, dest_path):
    """Download and extract the binary from the given URL."""
    try:
        import requests
        response = requests.get(url, headers={'User-Agent': 'uncomment-python-wrapper'}, allow_redirects=True)
        response.raise_for_status()

        with tempfile.NamedTemporaryFile(suffix='.tar.gz', delete=False) as tmp_file:
            tmp_file.write(response.content)
            tmp_file.flush()

            # Extract the binary from the tar.gz
            with tarfile.open(tmp_file.name, 'r:gz') as tar:
                # Find the binary file in the archive
                for member in tar.getmembers():
                    if member.name.endswith('uncomment') or member.name.endswith('uncomment.exe'):
                        # Extract to destination
                        with tar.extractfile(member) as binary_file:
                            with open(dest_path, 'wb') as f:
                                f.write(binary_file.read())
                        break
                else:
                    raise RuntimeError(f"No binary found in archive from {url}")

            # Clean up temp file
            os.unlink(tmp_file.name)

    except Exception as e:
        raise RuntimeError(f"Failed to download binary from {url}: {e}")


def get_binary_path():
    """Get the path where the binary should be stored."""
    cache_dir = Path.home() / ".cache" / "uncomment"
    cache_dir.mkdir(parents=True, exist_ok=True)

    ext = ".exe" if platform.system().lower() == "windows" else ""
    return cache_dir / f"uncomment{ext}"


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
