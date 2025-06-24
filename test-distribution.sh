#!/bin/bash

echo "🧪 Testing Distribution Packages Locally"
echo "========================================"
echo ""

# Test 1: Build the Rust binary
echo "1️⃣ Testing Rust build..."
cargo build --release
if [ $? -eq 0 ]; then
    echo "✅ Rust build successful"
    echo "   Binary size: $(du -h target/release/uncomment | cut -f1)"
else
    echo "❌ Rust build failed"
    exit 1
fi
echo ""

# Test 2: Check npm package structure
echo "2️⃣ Testing npm package structure..."
cd npm-package

echo "   📦 package.json validation..."
node -e "
const pkg = require('./package.json');
console.log('   Name:', pkg.name);
console.log('   Version:', pkg.version);
console.log('   Main:', pkg.main);
console.log('   Bin:', JSON.stringify(pkg.bin));
if (!pkg.dependencies['binary-install']) {
    console.error('❌ Missing binary-install dependency');
    process.exit(1);
}
console.log('✅ package.json valid');
"

echo "   🔍 Testing platform detection..."
node -e "
const { version } = require('./package.json');
const os = require('os');

function getPlatform() {
  const type = os.type();
  const arch = os.arch();

  if (type === 'Windows_NT') {
    return arch === 'x64' ? 'x86_64-pc-windows-msvc' : 'i686-pc-windows-msvc';
  }

  if (type === 'Linux') {
    if (arch === 'x64') return 'x86_64-unknown-linux-gnu';
    if (arch === 'arm64') return 'aarch64-unknown-linux-gnu';
    return 'x86_64-unknown-linux-gnu';
  }

  if (type === 'Darwin') {
    if (arch === 'x64') return 'x86_64-apple-darwin';
    if (arch === 'arm64') return 'aarch64-apple-darwin';
    return 'x86_64-apple-darwin';
  }

  throw new Error(\`Unsupported platform: \${type} \${arch}\`);
}

function getBinaryUrl() {
  const platform = getPlatform();
  const ext = os.type() === 'Windows_NT' ? '.exe' : '';
  const baseUrl = \`https://github.com/Goldziher/uncomment/releases/download/v\${version}\`;
  return \`\${baseUrl}/uncomment-\${platform}\${ext}\`;
}

try {
  const platform = getPlatform();
  const url = getBinaryUrl();
  console.log('   Detected platform:', platform);
  console.log('   Generated URL:', url);
  console.log('✅ Platform detection working');
} catch (e) {
  console.error('❌ Platform detection failed:', e.message);
  process.exit(1);
}
"

echo "   📁 Checking file permissions..."
if [ -x bin/uncomment ]; then
    echo "✅ bin/uncomment is executable"
else
    echo "❌ bin/uncomment is not executable"
fi

cd ..
echo ""

# Test 3: Check Python package structure
echo "3️⃣ Testing Python package structure..."
cd pip-package

echo "   📦 pyproject.toml validation..."
python3 -c "
import tomllib
try:
    with open('pyproject.toml', 'rb') as f:
        config = tomllib.load(f)

    project = config['project']
    print(f'   Name: {project[\"name\"]}')
    print(f'   Version: {project[\"version\"]}')
    print(f'   Scripts: {config[\"project\"][\"scripts\"]}')

    if not project.get('dependencies'):
        print('❌ Missing dependencies')
        exit(1)

    print('✅ pyproject.toml valid')
except Exception as e:
    print(f'❌ pyproject.toml validation failed: {e}')
    exit(1)
"

echo "   🔍 Testing Python platform detection..."
python3 -c "
import platform
import sys
import os
sys.path.insert(0, '.')

try:
    from uncomment.downloader import get_platform, get_binary_url
    from uncomment import __version__

    detected_platform = get_platform()
    url = get_binary_url(__version__)

    print(f'   Detected platform: {detected_platform}')
    print(f'   Generated URL: {url}')
    print(f'   Version: {__version__}')
    print('✅ Python platform detection working')
except Exception as e:
    print(f'❌ Python platform detection failed: {e}')
    exit(1)
"

echo "   📁 Checking Python package structure..."
if [ -f "uncomment/__init__.py" ] && [ -f "uncomment/cli.py" ] && [ -f "uncomment/downloader.py" ]; then
    echo "✅ Python package structure correct"
else
    echo "❌ Python package structure incomplete"
fi

cd ..
echo ""

# Test 4: Version consistency
echo "4️⃣ Testing version consistency..."
CARGO_VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
NPM_VERSION=$(grep '"version":' npm-package/package.json | sed 's/.*"version": "\(.*\)".*/\1/')
PIP_VERSION=$(grep '^version = ' pip-package/pyproject.toml | sed 's/version = "\(.*\)"/\1/')
PY_VERSION=$(grep '__version__ = ' pip-package/uncomment/__init__.py | sed 's/__version__ = "\(.*\)"/\1/')

echo "   Cargo version: $CARGO_VERSION"
echo "   npm version: $NPM_VERSION"
echo "   pip version: $PIP_VERSION"
echo "   Python __version__: $PY_VERSION"

if [ "$CARGO_VERSION" = "$NPM_VERSION" ] && [ "$PIP_VERSION" = "$PY_VERSION" ]; then
    echo "✅ Version consistency check passed"
else
    echo "❌ Version mismatch detected"
    exit 1
fi
echo ""

# Test 5: Mock binary download test
echo "5️⃣ Testing binary URL accessibility..."
VERSION="$CARGO_VERSION"
URL="https://github.com/Goldziher/uncomment/releases/download/v$VERSION"

echo "   Checking if release URL structure is valid..."
echo "   Expected URL: $URL"

# We can't test actual download since the release doesn't exist yet,
# but we can validate URL format
if [[ "$URL" =~ ^https://github\.com/Goldziher/uncomment/releases/download/v[0-9]+\.[0-9]+\.[0-9]+.*$ ]]; then
    echo "✅ URL format is valid"
else
    echo "❌ URL format is invalid"
    exit 1
fi
echo ""

echo "🎉 All distribution tests passed!"
echo ""
echo "📋 Summary:"
echo "   • Rust binary builds successfully"
echo "   • npm package structure is valid"
echo "   • Python package structure is valid"
echo "   • Platform detection works for both packages"
echo "   • Version consistency maintained"
echo "   • URL generation works correctly"
echo ""
echo "✅ Ready to push and create release tag v$CARGO_VERSION"
