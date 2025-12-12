const https = require("https");
const http = require("http");
const fs = require("fs");
const path = require("path");
const os = require("os");
const tar = require("tar");
const { version } = require("./package.json");

function getPlatform() {
  const type = os.type();
  const arch = os.arch();

  if (type === "Windows_NT") {
    return arch === "x64" ? "x86_64-pc-windows-msvc" : "i686-pc-windows-msvc";
  }

  if (type === "Linux") {
    if (arch === "x64") return "x86_64-unknown-linux-gnu";
    if (arch === "arm64") return "aarch64-unknown-linux-gnu";
    return "x86_64-unknown-linux-gnu";
  }

  if (type === "Darwin") {
    if (arch === "x64") return "x86_64-apple-darwin";
    if (arch === "arm64") return "aarch64-apple-darwin";
    return "x86_64-apple-darwin";
  }

  throw new Error(`Unsupported platform: ${type} ${arch}`);
}

function getBinaryUrl() {
  const platform = getPlatform();
  const baseUrl = `https://github.com/Goldziher/uncomment/releases/download/v${version}`;
  return `${baseUrl}/uncomment-${platform}.tar.gz`;
}

function downloadWithRedirects(url, dest, maxRedirects = 5) {
  return new Promise((resolve, reject) => {
    if (maxRedirects <= 0) {
      return reject(new Error("Too many redirects"));
    }

    const urlObj = new URL(url);
    const client = urlObj.protocol === "https:" ? https : http;

    const req = client.get(
      url,
      {
        headers: {
          "User-Agent": "uncomment-npm-wrapper",
        },
      },
      (res) => {
        if (
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location
        ) {
          return downloadWithRedirects(
            res.headers.location,
            dest,
            maxRedirects - 1,
          )
            .then(resolve)
            .catch(reject);
        }

        if (res.statusCode !== 200) {
          return reject(
            new Error(`HTTP ${res.statusCode}: ${res.statusMessage}`),
          );
        }

        const file = fs.createWriteStream(dest);
        res.pipe(file);

        file.on("finish", () => {
          file.close();
          resolve();
        });

        file.on("error", (err) => {
          fs.unlink(dest, () => {});
          reject(err);
        });
      },
    );

    req.on("error", reject);
    req.setTimeout(30000, () => {
      req.destroy();
      reject(new Error("Download timeout"));
    });
  });
}

async function installBinary() {
  try {
    const url = getBinaryUrl();
    const binDir = path.join(__dirname, "bin");
    const tarPath = path.join(binDir, "uncomment.tar.gz");
    const binaryName =
      os.type() === "Windows_NT" ? "uncomment.exe" : "uncomment";

    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    console.log(`Downloading uncomment binary from ${url}...`);

    await downloadWithRedirects(url, tarPath);

    console.log("Extracting binary...");

    await tar.extract({
      file: tarPath,
      cwd: binDir,
      filter: (path) => path.endsWith(binaryName),
    });

    fs.unlinkSync(tarPath);

    if (os.type() !== "Windows_NT") {
      const binaryPath = path.join(binDir, binaryName);
      fs.chmodSync(binaryPath, 0o755);
    }

    console.log("uncomment binary installed successfully!");
  } catch (error) {
    console.error("Error installing uncomment binary:", error.message);
    process.exit(1);
  }
}

installBinary();
