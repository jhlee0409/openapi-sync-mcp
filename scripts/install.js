#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

const PACKAGE = require('../package.json');
const VERSION = PACKAGE.version;
const REPO = 'jhlee0409/openapi-sync-mcp';

// Platform detection
const PLATFORM_MAP = {
  darwin: 'apple-darwin',
  linux: 'unknown-linux-gnu',
  win32: 'pc-windows-msvc',
};

const ARCH_MAP = {
  x64: 'x86_64',
  arm64: 'aarch64',
};

function getPlatformInfo() {
  const platform = PLATFORM_MAP[process.platform];
  const arch = ARCH_MAP[process.arch];

  if (!platform || !arch) {
    throw new Error(
      `Unsupported platform: ${process.platform}-${process.arch}\n` +
        'Supported: darwin-x64, darwin-arm64, linux-x64, linux-arm64, win32-x64'
    );
  }

  return {
    target: `${arch}-${platform}`,
    ext: process.platform === 'win32' ? '.exe' : '',
  };
}

function getBinaryName(info) {
  return `openapi-sync-mcp-${info.target}${info.ext}`;
}

function getDownloadUrl(binaryName) {
  return `https://github.com/${REPO}/releases/download/v${VERSION}/${binaryName}`;
}

async function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);

    const request = (url) => {
      https
        .get(url, (response) => {
          // Handle redirects
          if (response.statusCode === 302 || response.statusCode === 301) {
            request(response.headers.location);
            return;
          }

          if (response.statusCode !== 200) {
            reject(new Error(`Download failed: HTTP ${response.statusCode}`));
            return;
          }

          response.pipe(file);
          file.on('finish', () => {
            file.close();
            resolve();
          });
        })
        .on('error', (err) => {
          fs.unlink(dest, () => {});
          reject(err);
        });
    };

    request(url);
  });
}

async function main() {
  try {
    const info = getPlatformInfo();
    const binaryName = getBinaryName(info);
    const downloadUrl = getDownloadUrl(binaryName);
    const binDir = path.join(__dirname, '..', 'bin');
    const binaryPath = path.join(binDir, `openapi-sync-mcp${info.ext}`);

    // Ensure bin directory exists
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    console.log(`Downloading openapi-sync-mcp for ${info.target}...`);
    console.log(`URL: ${downloadUrl}`);

    await download(downloadUrl, binaryPath);

    // Make executable on Unix
    if (process.platform !== 'win32') {
      fs.chmodSync(binaryPath, 0o755);
    }

    console.log(`Successfully installed openapi-sync-mcp to ${binaryPath}`);
  } catch (error) {
    console.error('Failed to install openapi-sync-mcp binary:', error.message);
    console.error('');
    console.error('You can manually download from:');
    console.error(`https://github.com/${REPO}/releases/tag/v${VERSION}`);
    console.error('');
    console.error('Or build from source:');
    console.error('  git clone https://github.com/' + REPO);
    console.error('  cd openapi-sync-mcp && cargo build --release');
    process.exit(1);
  }
}

main();
