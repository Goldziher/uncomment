const crypto = require("node:crypto");
const fs = require("node:fs");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");
const tar = require("tar");
const AdmZip = require("adm-zip");

const { version } = require("./package.json");

const BASE_URL = `https://github.com/Goldziher/uncomment/releases/download/v${version}`;

// Must stay in step with the archives .github/workflows/publish.yaml uploads.
const TRIPLES = {
  "Windows_NT:x64": "x86_64-pc-windows-gnu",
  "Linux:x64": "x86_64-unknown-linux-gnu",
  "Linux:arm64": "aarch64-unknown-linux-gnu",
  "Darwin:x64": "x86_64-apple-darwin",
  "Darwin:arm64": "aarch64-apple-darwin",
};

function assertGlibc() {
  // The Linux archives are gnu-linked. On a musl host (Alpine) they resolve at
  // download time and then fail at exec with a bare "not found", so say so here.
  const report = typeof process.report?.getReport === "function" ? process.report.getReport() : null;
  if (report && report.header && report.header.glibcVersionRuntime) {
    return;
  }
  if (fs.existsSync("/etc/alpine-release")) {
    throw new Error(
      "musl-based Linux (Alpine) is not supported: only gnu-linked Linux archives are published. Build from source with `cargo install uncomment`.",
    );
  }
}

function getPlatformTriple() {
  const type = os.type();
  const arch = os.arch();
  const triple = TRIPLES[`${type}:${arch}`];

  if (!triple) {
    throw new Error(
      `No prebuilt uncomment binary for ${type} ${arch}. Supported: ${Object.keys(TRIPLES).join(", ")}. Build from source with \`cargo install uncomment\`.`,
    );
  }

  if (type === "Linux") {
    assertGlibc();
  }

  return triple;
}

function httpsGet(url, maxRedirects = 5) {
  return new Promise((resolve, reject) => {
    if (maxRedirects <= 0) {
      return reject(new Error("Too many redirects"));
    }

    // HTTPS only. Following a redirect down to plain HTTP would let a network
    // attacker substitute the archive that gets unpacked below.
    if (new URL(url).protocol !== "https:") {
      return reject(new Error(`Refusing non-HTTPS download URL: ${url}`));
    }

    const req = https.get(url, { headers: { "User-Agent": "uncomment-npm-wrapper" } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        res.resume();
        return httpsGet(new URL(res.headers.location, url).toString(), maxRedirects - 1)
          .then(resolve)
          .catch(reject);
      }

      if (res.statusCode !== 200) {
        res.resume();
        return reject(new Error(`HTTP ${res.statusCode} ${res.statusMessage} for ${url}`));
      }

      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    });

    req.on("error", reject);
    req.setTimeout(60000, () => {
      req.destroy(new Error(`Download timed out: ${url}`));
    });
  });
}

function expectedDigest(checksums, archiveName) {
  for (const line of checksums.toString("utf8").split("\n")) {
    const match = line.trim().match(/^([0-9a-f]{64})\s+\*?(\S+)$/);
    if (match && path.basename(match[2]) === archiveName) {
      return match[1];
    }
  }
  return null;
}

async function installBinary() {
  const triple = getPlatformTriple();
  const isWindows = os.type() === "Windows_NT";
  const archiveName = `uncomment-${triple}.${isWindows ? "zip" : "tar.gz"}`;

  const binDir = path.join(__dirname, "bin");
  // bin/uncomment is the committed launcher stub; the native executable lives
  // beside it under a distinct name so extraction never overwrites the stub.
  const binaryName = isWindows ? "uncomment.exe" : "uncomment-bin";
  const binaryPath = path.join(binDir, binaryName);
  const memberName = isWindows ? "uncomment.exe" : "uncomment";

  fs.mkdirSync(binDir, { recursive: true });

  if (fs.existsSync(binaryPath)) {
    return;
  }

  const stagingDir = fs.mkdtempSync(path.join(os.tmpdir(), "uncomment-install-"));

  try {
    console.log(`Downloading ${archiveName}...`);
    const archive = await httpsGet(`${BASE_URL}/${archiveName}`);

    // Verify against the release's own checksum manifest before unpacking. The
    // archive readers below (node-tar, adm-zip) have a history of path-traversal
    // issues, so nothing hostile should reach them in the first place.
    const checksums = await httpsGet(`${BASE_URL}/uncomment_${version}_checksums.txt`);
    const expected = expectedDigest(checksums, archiveName);
    if (!expected) {
      throw new Error(`No checksum published for ${archiveName}; refusing to install.`);
    }

    const actual = crypto.createHash("sha256").update(archive).digest("hex");
    if (actual !== expected) {
      throw new Error(`Checksum mismatch for ${archiveName}: expected ${expected}, got ${actual}`);
    }

    const archivePath = path.join(stagingDir, archiveName);
    fs.writeFileSync(archivePath, archive);

    console.log("Extracting...");
    if (isWindows) {
      const zip = new AdmZip(archivePath);
      const entry = zip.getEntries().find((e) => path.basename(e.entryName) === memberName);
      if (!entry) {
        throw new Error(`${memberName} not found in ${archiveName}`);
      }
      zip.extractEntryTo(entry, stagingDir, false, true);
    } else {
      await tar.extract({
        file: archivePath,
        cwd: stagingDir,
        filter: (entryPath) => path.basename(entryPath) === memberName,
      });
    }

    const extracted = path.join(stagingDir, memberName);
    if (!fs.existsSync(extracted)) {
      throw new Error(`${archiveName} did not contain ${memberName}`);
    }

    // copy + unlink rather than rename: the staging dir is in os.tmpdir(), which is
    // often a different filesystem from node_modules, and rename would fail EXDEV.
    fs.copyFileSync(extracted, binaryPath);
    fs.chmodSync(binaryPath, 0o755);
    console.log(`uncomment ${version} installed (${triple}).`);
  } finally {
    fs.rmSync(stagingDir, { recursive: true, force: true });
  }
}

installBinary().catch((error) => {
  console.error(`Failed to install the uncomment binary: ${error.message}`);
  process.exit(1);
});
