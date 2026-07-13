#!/usr/bin/env node
// Generates src-tauri/dmg/background.png — the branded DMG install window art.
// Renders a 660x420pt board at 2x (1320x840px), stamped to 144 DPI so Finder
// shows it retina-crisp at window size. Icon slots (app 170,220 / Applications
// 490,220 — see tauri.conf.json bundle.macOS.dmg) are left empty; Finder draws
// the real icons there.
//
// Usage: npm i --no-save playwright-core && node scripts/build-dmg-background.mjs
import { chromium } from 'playwright-core';
import { execFileSync } from 'node:child_process';
import { readFileSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const out = path.join(root, 'src-tauri', 'dmg', 'background.png');
const logoB64 = readFileSync(path.join(root, 'site', 'assets', 'donna-logo.png')).toString('base64');

// ponytail: hardcoded to the chromium already cached on this machine; point
// PLAYWRIGHT_CHROMIUM at another binary if that cache moves.
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM ||
  path.join(
    homedir(),
    'Library/Caches/ms-playwright/chromium-1208/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing'
  );

const html = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { width: 660px; height: 420px; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
    color: #2e241b;
    background-color: #f6f0e6;
    background-image: radial-gradient(#e7ddcf 1px, transparent 1px);
    background-size: 22px 22px;
    position: relative;
    overflow: hidden;
  }
  /* Section pill from the landing page (site .pill), face logo inside */
  .pill-row { position: absolute; top: 24px; left: 0; right: 0; display: flex; justify-content: center; }
  .pill {
    display: inline-flex; align-items: center; gap: 9px;
    background: #ffffff; border: 1px solid #e7ddcf; border-radius: 999px;
    padding: 5px 15px 5px 6px; box-shadow: 0 2px 8px rgba(46,36,27,0.05);
  }
  .pill img { width: 26px; height: 26px; border-radius: 8px; }
  .pill span { font-size: 13px; font-weight: 700; color: #2e241b; }
  /* Two-tone hero headline (site hero h1 + .muted split) */
  .headline {
    position: absolute; top: 78px; left: 0; right: 0; text-align: center;
    font-size: 34px; font-weight: 800; letter-spacing: -0.02em; line-height: 1.08;
    color: #2e241b;
  }
  .headline .muted { display: block; color: #92836f; }
  /* Miniature floating hero cards; clear of icon zones (170,220)/(490,220) and the arrow */
  .card {
    position: absolute; background: #ffffff; border-radius: 16px;
    box-shadow: 0 10px 30px rgba(46,36,27,0.08);
    padding: 11px 12px; width: 96px;
    display: flex; flex-direction: column; gap: 7px;
  }
  .card .label { font-size: 8px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; color: #92836f; }
  .card .bar { height: 6px; border-radius: 3px; background: #efe6d7; }
  .card .row { display: flex; align-items: center; gap: 6px; }
  .card .dot { width: 8px; height: 8px; border-radius: 50%; background: #9c5c33; flex: none; }
  .card.tl { left: 26px; top: 30px; transform: rotate(-4deg); }
  .card.br { right: 26px; bottom: 48px; transform: rotate(4deg); }
  .arrow { position: absolute; inset: 0; }
  .hint {
    position: absolute; top: 381px; left: 0; right: 0;
    text-align: center; font-size: 11px; color: #6e6257;
  }
  .hint code {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px; color: #9c5c33;
    background: #efe6d7; padding: 1px 5px; border-radius: 4px;
  }
</style>
</head>
<body>
  <div class="pill-row">
    <div class="pill">
      <img src="data:image/png;base64,${logoB64}" alt="">
      <span>Donna</span>
    </div>
  </div>
  <div class="headline">Drag Donna<span class="muted">into Applications</span></div>
  <div class="card tl">
    <div class="label">Today</div>
    <div class="bar" style="width: 100%"></div>
    <div class="bar" style="width: 68%"></div>
  </div>
  <div class="card br">
    <div class="row"><div class="dot"></div><div class="bar" style="flex: 1"></div></div>
    <div class="bar" style="width: 74%"></div>
  </div>
  <svg class="arrow" width="660" height="420" viewBox="0 0 660 420" xmlns="http://www.w3.org/2000/svg">
    <path d="M252 226 C 300 196, 368 194, 414 215"
          stroke="#9c5c33" stroke-width="7" fill="none" stroke-linecap="round"/>
    <g transform="translate(419 218) rotate(27)">
      <path d="M -15 -10 L 0 0 L -15 10"
            stroke="#9c5c33" stroke-width="7" fill="none"
            stroke-linecap="round" stroke-linejoin="round"/>
    </g>
  </svg>
  <div class="hint">macOS may call unsigned apps “damaged” — fix: open Terminal and run <code>xattr -cr /Applications/Donna.app</code></div>
</body>
</html>`;

mkdirSync(path.dirname(out), { recursive: true });
const browser = await chromium.launch({ executablePath });
const page = await browser.newPage({ viewport: { width: 660, height: 420 }, deviceScaleFactor: 2 });
await page.setContent(html, { waitUntil: 'load' });
await page.screenshot({ path: out });
await browser.close();
execFileSync('sips', ['-s', 'dpiWidth', '144', '-s', 'dpiHeight', '144', out], { stdio: 'ignore' });
console.log('wrote', out);
