#!/bin/sh
# shellcheck shell=dash
#
# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.

if [ "$KSH_VERSION" = 'Version JM 93t+ 2010-03-05' ]; then
    # The version of ksh93 that ships with many illumos systems does not
    # support the "local" extension. Print a message rather than fail in
    # subtle ways later on:
    echo 'this installer does not work with this ksh93 version; please try bash!' >&2
    exit 1
fi

set -u

APP_NAME="uncomment"
PRINT_VERBOSE=${INSTALLER_PRINT_VERBOSE:-0}
PRINT_QUIET=${INSTALLER_PRINT_QUIET:-0}
NO_MODIFY_PATH=${INSTALLER_NO_MODIFY_PATH:-0}
INSTALL_DIR="/usr/local/bin"

usage() {
    cat <<EOF
uncomment-installer.sh

The installer for uncomment

This script detects what platform you're on and fetches an appropriate archive from
the latest GitHub release, then unpacks the binary and installs it to

    $INSTALL_DIR

It will then add that directory to PATH by modifying your shell configuration files.

USAGE:
    ./install.sh [OPTIONS]

OPTIONS:
    -v, --verbose
            Enable verbose output

    -q, --quiet
            Disable progress output

        --no-modify-path
            Don't configure the PATH environment variable

    -h, --help
            Print help information
EOF
}

download_binary_and_run_installer() {
    downloader --check
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd tar
    need_cmd curl

    for arg in "$@"; do
        case "$arg" in
            --help)
                usage
                exit 0
                ;;
            --quiet)
                PRINT_QUIET=1
                ;;
            --verbose)
                PRINT_VERBOSE=1
                ;;
            --no-modify-path)
                NO_MODIFY_PATH=1
                ;;
            *)
                OPTIND=1
                if [ "${arg%%--*}" = "" ]; then
                    err "unknown option $arg"
                fi
                while getopts :hvq sub_arg "$arg"; do
                    case "$sub_arg" in
                        h)
                            usage
                            exit 0
                            ;;
                        v)
                            # user wants to skip the prompt --
                            # we don't need /dev/tty
                            PRINT_VERBOSE=1
                            ;;
                        q)
                            # user wants to skip the prompt --
                            # we don't need /dev/tty
                            PRINT_QUIET=1
                            ;;
                        *)
                            err "unknown option -$OPTARG"
                            ;;
                        esac
                done
                ;;
        esac
    done

    # Define repository details
    GITHUB_REPO="Goldziher/uncomment"

    say "Checking for latest version..."
    LATEST_VERSION=$(curl -s "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    say "Latest version: $LATEST_VERSION"

    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    # Determine target based on architecture
    local _artifact_name
    local _archive_ext
    local _bins
    local _updater_name=""
    local _updater_bin=""

    case "$_arch" in
        "aarch64-apple-darwin")
            _artifact_name="$APP_NAME-aarch64-apple-darwin-$LATEST_VERSION.tar.gz"
            _archive_ext=".tar.gz"
            _bins="$APP_NAME"
            ;;
        "x86_64-apple-darwin")
            _artifact_name="$APP_NAME-x86_64-apple-darwin-$LATEST_VERSION.tar.gz"
            _archive_ext=".tar.gz"
            _bins="$APP_NAME"
            ;;
        "x86_64-unknown-linux-gnu")
            _artifact_name="$APP_NAME-x86_64-unknown-linux-gnu-$LATEST_VERSION.tar.gz"
            _archive_ext=".tar.gz"
            _bins="$APP_NAME"
            ;;
        "x86_64-pc-windows-gnu")
            _artifact_name="$APP_NAME-x86_64-pc-windows-msvc-$LATEST_VERSION.zip"
            _archive_ext=".zip"
            _bins="$APP_NAME.exe"
            ;;
        *)
            err "Unsupported architecture: $_arch"
            ;;
    esac

    # Construct download URL
    local _url="https://github.com/$GITHUB_REPO/releases/download/$LATEST_VERSION/$_artifact_name"
    local _dir
    if ! _dir="$(ensure mktemp -d)"; then
        # Because the previous command ran in a subshell, we must manually
        # propagate exit status.
        exit 1
    fi
    local _file="$_dir/input$_archive_ext"

    say "downloading $APP_NAME $LATEST_VERSION for $_arch" 1>&2
    say_verbose "  from $_url" 1>&2
    say_verbose "  to $_file" 1>&2

    ensure mkdir -p "$_dir"

    if ! downloader "$_url" "$_file"; then
      say "failed to download $_url"
      say "this may be a standard network error, but it may also indicate"
      say "that $APP_NAME's release process is not working."
      exit 1
    fi

    # unpack the archive
    case "$_archive_ext" in
        ".zip")
            ensure unzip -q "$_file" -d "$_dir"
            ;;
        ".tar"*)
            ensure tar xf "$_file" -C "$_dir"
            ;;
        *)
            err "unknown archive format: $_archive_ext"
            ;;
    esac

    # Install binary
    say "installing to $INSTALL_DIR/$APP_NAME..."
    if [ -w "$INSTALL_DIR" ]; then
        ensure mv "$_dir/$_bins" "$INSTALL_DIR/"
    else
        # Need sudo to write to install directory
        say "Elevated permissions required to install to $INSTALL_DIR"
        ensure sudo mv "$_dir/$_bins" "$INSTALL_DIR/"
    fi

    # Make binary executable
    if [ -w "$INSTALL_DIR/$_bins" ]; then
        ensure chmod +x "$INSTALL_DIR/$_bins"
    else
        ensure sudo chmod +x "$INSTALL_DIR/$_bins"
    fi

    # Clean up
    ignore rm -rf "$_dir"

    # Verify installation
    if command -v "$APP_NAME" > /dev/null; then
        say "$APP_NAME $LATEST_VERSION has been installed successfully!"
        say "Run '$APP_NAME --help' to get started."
    else
        say "Installation failed. Please check if $INSTALL_DIR is in your PATH."
        exit 1
    fi

    # PATH configuration
    if [ "0" = "$NO_MODIFY_PATH" ]; then
        add_install_dir_to_path "$INSTALL_DIR" "$INSTALL_DIR/$APP_NAME-env" "$INSTALL_DIR/$APP_NAME-env" ".profile" "sh"
        exit1=$?
        add_install_dir_to_path "$INSTALL_DIR" "$INSTALL_DIR/$APP_NAME-env" "$INSTALL_DIR/$APP_NAME-env" ".bash_profile .bash_login .bashrc" "sh"
        exit2=$?
        add_install_dir_to_path "$INSTALL_DIR" "$INSTALL_DIR/$APP_NAME-env" "$INSTALL_DIR/$APP_NAME-env" ".zshrc .zshenv" "sh"
        exit3=$?
        # This path may not exist by default
        ensure mkdir -p "$HOME/.config/fish/conf.d"
        exit4=$?
        add_install_dir_to_path "$INSTALL_DIR" "$INSTALL_DIR/$APP_NAME-env.fish" "$INSTALL_DIR/$APP_NAME-env.fish" ".config/fish/conf.d/$APP_NAME.env.fish" "fish"
        exit5=$?

        if [ "${exit1:-0}" = 1 ] || [ "${exit2:-0}" = 1 ] || [ "${exit3:-0}" = 1 ] || [ "${exit4:-0}" = 1 ] || [ "${exit5:-0}" = 1 ]; then
            say ""
            say "To add $INSTALL_DIR to your PATH, either restart your shell or run:"
            say ""
            say "    source $INSTALL_DIR/$APP_NAME-env (sh, bash, zsh)"
            say "    source $INSTALL_DIR/$APP_NAME-env.fish (fish)"
        fi
    fi

    return 0
}

add_install_dir_to_path() {
    # Edit rcfiles ($HOME/.profile) to add install_dir to $PATH
    local _install_dir_expr="$1"
    local _env_script_path="$2"
    local _env_script_path_expr="$3"
    local _rcfiles="$4"
    local _shell="$5"

    if [ -n "${HOME:-}" ]; then
        local _target
        local _home

        # Find the first file in the array that exists and choose
        # that as our target to write to
        for _rcfile_relative in $_rcfiles; do
            _home="$(print_home_for_script "$_rcfile_relative")"
            local _rcfile="$_home/$_rcfile_relative"

            if [ -f "$_rcfile" ]; then
                _target="$_rcfile"
                break
            fi
        done

        # If we didn't find anything, pick the first entry in the
        # list as the default to create and write to
        if [ -z "${_target:-}" ]; then
            local _rcfile_relative
            _rcfile_relative="$(echo "$_rcfiles" | awk '{ print $1 }')"
            _home="$(print_home_for_script "$_rcfile_relative")"
            _target="$_home/$_rcfile_relative"
        fi

        # `source x` is an alias for `. x`, and the latter is more portable/actually-posix.
        local _robust_line=". \"$_env_script_path_expr\""
        local _pretty_line="source \"$_env_script_path_expr\""

        # Add the env script if it doesn't already exist
        if [ ! -f "$_env_script_path" ]; then
            say_verbose "creating $_env_script_path"
            if [ "$_shell" = "sh" ]; then
                write_env_script_sh "$_install_dir_expr" "$_env_script_path"
            else
                write_env_script_fish "$_install_dir_expr" "$_env_script_path"
            fi
        else
            say_verbose "$_env_script_path already exists"
        fi

        # Check if the line is already in the rcfile
        if ! grep -F "$_robust_line" "$_target" > /dev/null 2>/dev/null && \
           ! grep -F "$_pretty_line" "$_target" > /dev/null 2>/dev/null
        then
            # If the script now exists, add the line to source it to the rcfile
            # (This will also create the rcfile if it doesn't exist)
            if [ -f "$_env_script_path" ]; then
                local _line
                # Fish has deprecated `.` as an alias for `source` and
                # it will be removed in a later version.
                if [ "$_shell" = "fish" ]; then
                    _line="$_pretty_line"
                else
                    _line="$_robust_line"
                fi
                say_verbose "adding $_line to $_target"
                # prepend an extra newline in case the user's file is missing a trailing one
                ensure echo "" >> "$_target"
                ensure echo "$_line" >> "$_target"
                return 1
            fi
        else
            say_verbose "$_install_dir_expr already on PATH"
        fi
    fi
}

print_home_for_script() {
    local script="$1"

    local _home
    case "$script" in
        # zsh has a special ZDOTDIR directory, which if set
        # should be considered instead of $HOME
        .zsh*)
            if [ -n "${ZDOTDIR:-}" ]; then
                _home="$ZDOTDIR"
            else
                _home="$HOME"
            fi
            ;;
        *)
            _home="$HOME"
            ;;
    esac

    echo "$_home"
}

write_env_script_sh() {
    # write this env script to the given path
    local _install_dir_expr="$1"
    local _env_script_path="$2"
    ensure cat <<EOF > "$_env_script_path"
#!/bin/sh
# add binaries to PATH if they aren't added yet
# affix colons on either side of \$PATH to simplify matching
case ":\${PATH}:" in
    *:"$_install_dir_expr":*)
        ;;
    *)
        # Prepending path in case a system-installed binary needs to be overridden
        export PATH="$_install_dir_expr:\$PATH"
        ;;
esac
EOF
    # Make the script executable
    ensure chmod +x "$_env_script_path"
}

write_env_script_fish() {
    # write this env script to the given path
    local _install_dir_expr="$1"
    local _env_script_path="$2"
    ensure cat <<EOF > "$_env_script_path"
if not contains "$_install_dir_expr" \$PATH
    # Prepending path in case a system-installed binary needs to be overridden
    set -x PATH "$_install_dir_expr" \$PATH
end
EOF
    # Make the script executable
    ensure chmod +x "$_env_script_path"
}

check_proc() {
    # Check for /proc by looking for the /proc/self/exe link
    # This is only run on Linux
    if ! test -L /proc/self/exe ; then
        err "fatal: Unable to find /proc/self/exe. Is /proc mounted? Installation cannot proceed without /proc."
    fi
}

get_bitness() {
    need_cmd head
    # Architecture detection without dependencies beyond coreutils.
    # ELF files start out "\x7fELF", and the following byte is
    #   0x01 for 32-bit and
    #   0x02 for 64-bit.
    # The printf builtin on some shells like dash only supports octal
    # escape sequences, so we use those.
    local _current_exe_head
    _current_exe_head=$(head -c 5 /proc/self/exe )
    if [ "$_current_exe_head" = "$(printf '\177ELF\001')" ]; then
        echo 32
    elif [ "$_current_exe_head" = "$(printf '\177ELF\002')" ]; then
        echo 64
    else
        err "unknown platform bitness"
    fi
}

is_host_amd64_elf() {
    need_cmd head
    need_cmd tail
    # ELF e_machine detection without dependencies beyond coreutils.
    # Two-byte field at offset 0x12 indicates the CPU,
    # but we're interested in it being 0x3E to indicate amd64, or not that.
    local _current_exe_machine
    _current_exe_machine=$(head -c 19 /proc/self/exe | tail -c 1)
    [ "$_current_exe_machine" = "$(printf '\076')" ]
}

get_endianness() {
    local cputype=$1
    local suffix_eb=$2
    local suffix_el=$3

    # detect endianness without od/hexdump, like get_bitness() does.
    need_cmd head
    need_cmd tail

    local _current_exe_endianness
    _current_exe_endianness="$(head -c 6 /proc/self/exe | tail -c 1)"
    if [ "$_current_exe_endianness" = "$(printf '\001')" ]; then
        echo "${cputype}${suffix_el}"
    elif [ "$_current_exe_endianness" = "$(printf '\002')" ]; then
        echo "${cputype}${suffix_eb}"
    else
        err "unknown platform endianness"
    fi
}

get_architecture() {
    local _ostype
    local _cputype
    _ostype="$(uname -s)"
    _cputype="$(uname -m)"
    local _clibtype="gnu"

    if [ "$_ostype" = Linux ]; then
        if [ "$(uname -o)" = Android ]; then
            _ostype=Android
        fi
        if ldd --version 2>&1 | grep -q 'musl'; then
            _clibtype="musl-dynamic"
        fi
    fi

    if [ "$_ostype" = Darwin ] && [ "$_cputype" = i386 ]; then
        # Darwin `uname -m` lies
        if sysctl hw.optional.x86_64 | grep -q ': 1'; then
            _cputype=x86_64
        fi
    fi

    if [ "$_ostype" = Darwin ] && [ "$_cputype" = x86_64 ]; then
        # Rosetta on aarch64
        if [ "$(sysctl -n hw.optional.arm64 2>/dev/null)" = "1" ]; then
            _cputype=aarch64
        fi
    fi

    if [ "$_ostype" = SunOS ]; then
        # Both Solaris and illumos presently announce as "SunOS" in "uname -s"
        # so use "uname -o" to disambiguate.  We use the full path to the
        # system uname in case the user has coreutils uname first in PATH,
        # which has historically sometimes printed the wrong value here.
        if [ "$(/usr/bin/uname -o)" = illumos ]; then
            _ostype=illumos
        fi

        # illumos systems have multi-arch userlands, and "uname -m" reports the
        # machine hardware name; e.g., "i86pc" on both 32- and 64-bit x86
        # systems.  Check for the native (widest) instruction set on the
        # running kernel:
        if [ "$_cputype" = i86pc ]; then
            _cputype="$(isainfo -n)"
        fi
    fi

    case "$_ostype" in
        Android)
            _ostype=linux-android
            ;;
        Linux)
            check_proc
            _ostype=unknown-linux-$_clibtype
            _bitness=$(get_bitness)
            ;;
        FreeBSD)
            _ostype=unknown-freebsd
            ;;
        NetBSD)
            _ostype=unknown-netbsd
            ;;
        DragonFly)
            _ostype=unknown-dragonfly
            ;;
        Darwin)
            _ostype=apple-darwin
            ;;
        illumos)
            _ostype=unknown-illumos
            ;;
        MINGW* | MSYS* | CYGWIN* | Windows_NT)
            _ostype=pc-windows-gnu
            ;;
        *)
            err "unrecognized OS type: $_ostype"
            ;;
    esac

    case "$_cputype" in
        i386 | i486 | i686 | i786 | x86)
            _cputype=i686
            ;;
        xscale | arm)
            _cputype=arm
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            fi
            ;;
        armv6l)
            _cputype=arm
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            else
                _ostype="${_ostype}eabihf"
            fi
            ;;
        armv7l | armv8l)
            _cputype=armv7
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            else
                _ostype="${_ostype}eabihf"
            fi
            ;;
        aarch64 | arm64)
            _cputype=aarch64
            ;;
        x86_64 | x86-64 | x64 | amd64)
            _cputype=x86_64
            ;;
        mips)
            _cputype=$(get_endianness mips '' el)
            ;;
        mips64)
            if [ "$_bitness" -eq 64 ]; then
                # only n64 ABI is supported for now
                _ostype="${_ostype}abi64"
                _cputype=$(get_endianness mips64 '' el)
            fi
            ;;
        ppc)
            _cputype=powerpc
            ;;
        ppc64)
            _cputype=powerpc64
            ;;
        ppc64le)
            _cputype=powerpc64le
            ;;
        s390x)
            _cputype=s390x
            ;;
        riscv64)
            _cputype=riscv64gc
            ;;
        loongarch64)
            _cputype=loongarch64
            ;;
        *)
            err "unknown CPU type: $_cputype"
    esac

    # Detect 64-bit linux with 32-bit userland
    if [ "${_ostype}" = unknown-linux-gnu ] && [ "${_bitness}" -eq 32 ]; then
        case $_cputype in
            x86_64)
                # 32-bit executable for amd64 = x32
                if is_host_amd64_elf; then {
                    err "x32 linux unsupported"
                }; else
                    _cputype=i686
                fi
                ;;
            mips64)
                _cputype=$(get_endianness mips '' el)
                ;;
            powerpc64)
                _cputype=powerpc
                ;;
            aarch64)
                _cputype=armv7
                if [ "$_ostype" = "linux-android" ]; then
                    _ostype=linux-androideabi
                else
                    _ostype="${_ostype}eabihf"
                fi
                ;;
            riscv64gc)
                err "riscv64 with 32-bit userland unsupported"
                ;;
        esac
    fi

    # treat armv7 systems without neon as plain arm
    if [ "$_ostype" = "unknown-linux-gnueabihf" ] && [ "$_cputype" = armv7 ]; then
        if ensure grep '^Features' /proc/cpuinfo | grep -q -v neon; then
            # At least one processor does not have NEON.
            _cputype=arm
        fi
    fi

    _arch="${_cputype}-${_ostype}"

    RETVAL="$_arch"
}

say() {
    if [ "0" = "$PRINT_QUIET" ]; then
        echo "$1"
    fi
}

say_verbose() {
    if [ "1" = "$PRINT_VERBOSE" ]; then
        echo "$1"
    fi
}

err() {
    if [ "0" = "$PRINT_QUIET" ]; then
        local red
        local reset
        red=$(tput setaf 1 2>/dev/null || echo '')
        reset=$(tput sgr0 2>/dev/null || echo '')
        say "${red}ERROR${reset}: $1" >&2
    fi
    exit 1
}

need_cmd() {
    if ! check_cmd "$1"
    then err "need '$1' (command not found)"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
    return $?
}

assert_nz() {
    if [ -z "$1" ]; then err "assert_nz $2"; fi
}

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing command.
ensure() {
    if ! "$@"; then err "command failed: $*"; fi
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
    "$@"
}

# This wraps curl or wget. Try curl first, if not installed,
# use wget instead.
downloader() {
    if check_cmd curl
    then _dld=curl
    elif check_cmd wget
    then _dld=wget
    else _dld='curl or wget' # to be used in error message of need_cmd
    fi

    if [ "$1" = --check ]
    then need_cmd "$_dld"
    elif [ "$_dld" = curl ]
    then curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]
    then wget "$1" -O "$2"
    else err "Unknown downloader"   # should not reach here
    fi
}

download_binary_and_run_installer "$@" || exit 1
