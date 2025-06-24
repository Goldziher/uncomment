const { Binary } = require("binary-install");
const os = require("os");
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
    return "x86_64-unknown-linux-gnu"; // fallback
  }

  if (type === "Darwin") {
    if (arch === "x64") return "x86_64-apple-darwin";
    if (arch === "arm64") return "aarch64-apple-darwin";
    return "x86_64-apple-darwin"; // fallback
  }

  throw new Error(`Unsupported platform: ${type} ${arch}`);
}

function getBinaryUrl() {
  const platform = getPlatform();
  const ext = os.type() === "Windows_NT" ? ".exe" : "";
  const baseUrl = `https://github.com/Goldziher/uncomment/releases/download/v${version}`;
  return `${baseUrl}/uncomment-${platform}${ext}`;
}

const binary = new Binary("uncomment", getBinaryUrl());

module.exports = binary;
