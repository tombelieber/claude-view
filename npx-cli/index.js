#!/usr/bin/env node

const { execFileSync, spawn } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const https = require("https");
const zlib = require("zlib");

const VERSION = require("./package.json").version;
const REPO = "OWNER/claude-view";
const BINARY_NAME = process.platform === "win32" ? "vibe-recall.exe" : "vibe-recall";

// --- Platform detection ---

const PLATFORM_MAP = {
  "darwin-arm64": { artifact: "claude-view-darwin-arm64.tar.gz", ext: "tar.gz" },
  "darwin-x64": { artifact: "claude-view-darwin-x64.tar.gz", ext: "tar.gz" },
  "linux-x64": { artifact: "claude-view-linux-x64.tar.gz", ext: "tar.gz" },
  "win32-x64": { artifact: "claude-view-win32-x64.zip", ext: "zip" },
};

const platformKey = `${process.platform}-${process.arch}`;
const platformInfo = PLATFORM_MAP[platformKey];

if (!platformInfo) {
  console.error(
    `Error: Unsupported platform "${process.platform}" with architecture "${process.arch}".\n` +
      `Supported: macOS (arm64, x64), Linux (x64), Windows (x64).`
  );
  process.exit(1);
}

// --- Cache paths ---

const cacheDir = path.join(os.homedir(), ".cache", "claude-view");
const binDir = path.join(cacheDir, "bin");
const versionFile = path.join(cacheDir, "version");
const binaryPath = path.join(binDir, BINARY_NAME);
const distDir = path.join(binDir, "dist");

// --- Helpers ---

function download(url) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, (res) => {
      // Follow redirects (GitHub releases redirect to S3/CDN)
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return download(res.headers.location).then(resolve, reject);
      }
      if (res.statusCode !== 200) {
        reject(new Error(`Download failed: HTTP ${res.statusCode} from ${url}`));
        res.resume();
        return;
      }
      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    });
    request.on("error", reject);
  });
}

function extractTarGz(buffer, destDir) {
  // Use system tar — available on macOS, Linux, and modern Windows (tar ships with Win10+)
  fs.mkdirSync(destDir, { recursive: true });
  const tmpFile = path.join(os.tmpdir(), `claude-view-${Date.now()}.tar.gz`);
  fs.writeFileSync(tmpFile, buffer);
  try {
    execFileSync("tar", ["xzf", tmpFile, "-C", destDir], { stdio: "pipe" });
  } finally {
    fs.unlinkSync(tmpFile);
  }
}

function extractZip(buffer, destDir) {
  // Use system tar on Windows 10+ (supports zip) or PowerShell as fallback
  fs.mkdirSync(destDir, { recursive: true });
  const tmpFile = path.join(os.tmpdir(), `claude-view-${Date.now()}.zip`);
  fs.writeFileSync(tmpFile, buffer);
  try {
    if (process.platform === "win32") {
      execFileSync(
        "powershell",
        ["-Command", `Expand-Archive -Force -Path '${tmpFile}' -DestinationPath '${destDir}'`],
        { stdio: "pipe" }
      );
    } else {
      execFileSync("unzip", ["-o", tmpFile, "-d", destDir], { stdio: "pipe" });
    }
  } finally {
    fs.unlinkSync(tmpFile);
  }
}

function downloadChecksums(version) {
  const url = `https://github.com/${REPO}/releases/download/v${version}/checksums.txt`;
  return download(url)
    .then((buf) => {
      const map = {};
      const lines = buf.toString("utf-8").split("\n");
      for (const line of lines) {
        // Format: "<64-hex-chars>  <filename>" (two spaces between hash and filename)
        const match = line.match(/^([0-9a-f]{64})  (.+)$/);
        if (match) {
          map[match[2]] = match[1];
        }
      }
      return map;
    })
    .catch(() => null); // Graceful fallback for older releases without checksums
}

function verifyChecksum(buffer, expectedHash) {
  const crypto = require("crypto");
  const actualHash = crypto.createHash("sha256").update(buffer).digest("hex");
  if (actualHash !== expectedHash) {
    console.error(`Checksum verification failed.`);
    console.error(`  Expected: ${expectedHash}`);
    console.error(`  Actual:   ${actualHash}`);
    process.exit(1);
  }
}

// --- Main ---

async function main() {
  // Check if cached version matches
  let needsDownload = true;
  if (fs.existsSync(versionFile) && fs.existsSync(binaryPath)) {
    const cached = fs.readFileSync(versionFile, "utf-8").trim();
    if (cached === VERSION) {
      needsDownload = false;
    }
  }

  if (needsDownload) {
    const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${platformInfo.artifact}`;
    console.log(`Downloading claude-view v${VERSION} for ${platformKey}...`);

    let buffer;
    try {
      buffer = await download(url);
    } catch (err) {
      console.error(`\nFailed to download claude-view:\n  ${err.message}`);
      console.error(`\nURL: ${url}`);
      console.error(`\nCheck that release v${VERSION} exists at https://github.com/${REPO}/releases`);
      process.exit(1);
    }

    // Verify checksum if available
    const checksums = await downloadChecksums(VERSION);
    if (checksums && checksums[platformInfo.artifact]) {
      verifyChecksum(buffer, checksums[platformInfo.artifact]);
      console.log("Checksum verified.");
    }

    // Clean previous install
    fs.rmSync(binDir, { recursive: true, force: true });
    fs.mkdirSync(binDir, { recursive: true });

    // Extract
    try {
      if (platformInfo.ext === "zip") {
        extractZip(buffer, binDir);
      } else {
        extractTarGz(buffer, binDir);
      }
    } catch (err) {
      console.error(`\nFailed to extract archive:\n  ${err.message}`);
      process.exit(1);
    }

    // Make binary executable (no-op on Windows)
    if (process.platform !== "win32") {
      fs.chmodSync(binaryPath, 0o755);
    }

    // Write version marker
    fs.mkdirSync(cacheDir, { recursive: true });
    fs.writeFileSync(versionFile, VERSION);

    console.log(`Installed to ${binDir}`);
  }

  // Verify binary exists
  if (!fs.existsSync(binaryPath)) {
    console.error(`Error: Binary not found at ${binaryPath}`);
    console.error("Try deleting ~/.cache/claude-view/ and running again.");
    process.exit(1);
  }

  // Set STATIC_DIR so the server finds the frontend assets
  const env = { ...process.env, STATIC_DIR: distDir };

  // Run the server, forwarding signals and exit code
  const child = spawn(binaryPath, process.argv.slice(2), { stdio: "inherit", env });

  // Forward signals to child process
  for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    process.on(sig, () => child.kill(sig));
  }

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code ?? 1);
    }
  });
}

main();
