"""
CLI entry point for uncomment
"""

import sys
from .downloader import run_uncomment


def main():
    """Main entry point for the CLI."""
    # Pass all arguments except the script name to the binary
    args = sys.argv[1:]
    run_uncomment(args)


if __name__ == "__main__":
    main()
