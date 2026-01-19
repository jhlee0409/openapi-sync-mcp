#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const ext = process.platform === 'win32' ? '.exe' : '';
const binaryPath = path.join(__dirname, `openapi-sync-mcp${ext}`);

// Check if binary exists
if (!fs.existsSync(binaryPath)) {
  console.error('Error: openapi-sync-mcp binary not found.');
  console.error('');
  console.error('Try reinstalling the package:');
  console.error('  npm install -g @jhlee0409/openapi-sync-mcp');
  console.error('');
  console.error('Or run the install script manually:');
  console.error('  node ' + path.join(__dirname, '..', 'scripts', 'install.js'));
  process.exit(1);
}

// Spawn the binary with all arguments
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  env: process.env,
});

child.on('error', (err) => {
  console.error('Failed to start openapi-sync-mcp:', err.message);
  process.exit(1);
});

child.on('close', (code) => {
  process.exit(code ?? 0);
});
