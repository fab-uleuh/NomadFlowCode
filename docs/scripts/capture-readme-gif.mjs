#!/usr/bin/env node
/**
 * capture-readme-gif.mjs
 *
 * Generates the animated GIF used in the README.
 * It opens docs/public/readme-animation.html in a real Chrome window,
 * captures screenshots at regular intervals, then assembles them into
 * a GIF with ffmpeg.
 *
 * ── Prerequisites ──────────────────────────────────────────────────
 *   brew install ffmpeg          # GIF assembly
 *   cd docs && pnpm install      # docs dev dependencies
 *   npm install -g playwright    # or: npm install playwright
 *   npx playwright install chromium
 *
 * ── Usage ──────────────────────────────────────────────────────────
 *   # 1. Start the docs dev server (in another terminal)
 *   cd docs && pnpm dev
 *
 *   # 2. Run this script
 *   node docs/scripts/capture-readme-gif.mjs
 *
 *   # Output: docs/public/demo-cli.gif
 *
 * ── What it captures ───────────────────────────────────────────────
 *   Left side:  Terminal animation (setup wizard → link → serve + QR)
 *   Right side: Phone mockup playing docs/public/demo.mp4 (starts at 8s)
 *
 * ── How to update ──────────────────────────────────────────────────
 *   • To change the CLI flow:  edit docs/public/readme-animation.html
 *     - WIZARD array: setup wizard screens (password, tunnel, subdomain, confirm)
 *     - SHELL array:  typed commands and their output
 *   • To change the phone video: replace docs/public/demo.mp4
 *     - Adjust video.currentTime in readme-animation.html if the
 *       interesting part doesn't start at 8s
 *   • To change capture settings: edit the constants below
 */

import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const { execSync } = require('child_process');
const { mkdirSync, rmSync, statSync } = require('fs');

// ── Configuration ──────────────────────────────────────────────────

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const DOCS_DIR = resolve(SCRIPT_DIR, '..');
const OUTPUT_GIF = resolve(DOCS_DIR, 'public/demo-cli.gif');
const FRAMES_DIR = '/tmp/nomadflow-gif-frames';

const DEV_SERVER_URL = 'http://localhost:3000/readme-animation.html';
const FPS = 8;              // frames per second during capture
const DURATION_S = 17;      // total capture duration in seconds
const GIF_WIDTH = 800;      // final GIF width in pixels
const GIF_MAX_COLORS = 128; // max palette colors (lower = smaller file)
const VIEWPORT = { width: 1200, height: 680 };

// ── Main ───────────────────────────────────────────────────────────

async function main() {
  // Verify dev server is running
  try {
    const res = await fetch(DEV_SERVER_URL);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
  } catch {
    console.error('Error: docs dev server not running.');
    console.error('Start it first:  cd docs && pnpm dev');
    process.exit(1);
  }

  // Prepare frames directory
  rmSync(FRAMES_DIR, { recursive: true, force: true });
  mkdirSync(FRAMES_DIR, { recursive: true });

  // Launch headed Chrome (video needs a real window to play)
  console.log('Launching Chrome...');
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage();
  await page.setViewportSize(VIEWPORT);
  await page.goto(DEV_SERVER_URL, { waitUntil: 'domcontentloaded' });

  // Wait for the animation script to be ready
  await page.waitForFunction(() => window.__animationReady === true, { timeout: 5000 });

  // Let the video buffer and start playing
  await page.waitForTimeout(1000);

  // Capture frames
  const totalFrames = FPS * DURATION_S;
  const interval = 1000 / FPS;
  console.log(`Capturing ${totalFrames} frames at ${FPS} FPS (${DURATION_S}s)...`);

  for (let i = 0; i < totalFrames; i++) {
    const padded = String(i).padStart(4, '0');
    await page.screenshot({ path: `${FRAMES_DIR}/frame-${padded}.png`, type: 'png' });
    await page.waitForTimeout(interval);
  }

  console.log('Frames captured. Closing browser...');
  await browser.close();

  // Assemble GIF with ffmpeg (two-pass for best quality)
  console.log('Assembling GIF with ffmpeg...');
  execSync(`
    cd "${FRAMES_DIR}" && \
    ffmpeg -y -framerate ${FPS} -i frame-%04d.png \
      -vf "fps=6,scale=${GIF_WIDTH}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=${GIF_MAX_COLORS}:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=4" \
      -loop 0 \
      "${OUTPUT_GIF}"
  `, { stdio: 'inherit' });

  // Report
  const size = statSync(OUTPUT_GIF).size;
  console.log(`\nDone! GIF saved to: ${OUTPUT_GIF}`);
  console.log(`Size: ${(size / 1024).toFixed(0)} KB`);

  // Cleanup
  rmSync(FRAMES_DIR, { recursive: true, force: true });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
