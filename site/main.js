// OS-aware download button, per-platform direct links, and the latest version —
// all from the GitHub Releases API. Every link downloads the installer directly;
// nobody has to pick through the release's asset list.
const REPO = "duckyquang/Donna";

function detectOS() {
  if (/iPhone|iPad|Android/i.test(navigator.userAgent)) return "other";
  const p = `${navigator.platform} ${navigator.userAgent}`;
  if (/Mac/i.test(p)) return "mac";
  if (/Win/i.test(p)) return "windows";
  if (/Linux/i.test(p)) return "linux";
  return "other";
}

const OS_LABEL = {
  mac: "Download for macOS",
  windows: "Download for Windows",
  linux: "Download for Linux",
  other: "Download Donna",
};

async function latestAssets() {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`);
  if (!res.ok) return null;
  const rel = await res.json();
  const find = (re) => rel.assets.find((a) => re.test(a.name))?.browser_download_url;
  return {
    version: rel.tag_name,
    mac: find(/aarch64\.dmg$/) || find(/\.dmg$/),
    macIntel: find(/x64\.dmg$/),
    windows: find(/set(up)?\.exe$/i) || find(/\.msi$/) || find(/\.exe$/),
    linux: find(/\.AppImage$/) || find(/\.deb$/),
    linuxDeb: find(/\.deb$/),
  };
}

(async () => {
  const os = detectOS();
  const btn = document.querySelector("[data-download]");
  if (btn) btn.textContent = OS_LABEL[os];
  const assets = await latestAssets().catch(() => null);
  if (!assets) return; // links already point at the releases page
  if (btn && assets[os]) btn.href = assets[os];

  const v = document.querySelector("[data-version]");
  if (v && assets.version) v.textContent = `· ${assets.version}`;

  const intel = document.querySelector("[data-mac-intel]");
  if (intel && os === "mac" && assets.macIntel) {
    intel.href = assets.macIntel;
    intel.hidden = false;
  }
})();

// Gentle scroll reveals with per-group stagger. Decorative only — content is
// visible without JS, and reduced-motion drops the movement in CSS.
(() => {
  const revealables = document.querySelectorAll(".reveal");
  if (!("IntersectionObserver" in window)) {
    for (const el of revealables) el.classList.add("is-visible");
    return;
  }
  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          io.unobserve(entry.target);
        }
      }
    },
    { rootMargin: "0px 0px -60px 0px" },
  );
  for (const el of revealables) {
    const group = el.parentElement.querySelectorAll(":scope > .reveal");
    const idx = Array.prototype.indexOf.call(group, el);
    el.style.setProperty("--reveal-delay", `${Math.min(idx, 6) * 60}ms`);
    io.observe(el);
  }
})();

// Copy-to-clipboard buttons (FAQ terminal command).
(() => {
  if (!navigator.clipboard?.writeText) return;
  for (const btn of document.querySelectorAll(".copy-cmd")) {
    const label = btn.textContent;
    btn.addEventListener("click", () => {
      navigator.clipboard.writeText(btn.dataset.copy).then(() => {
        btn.textContent = "Copied";
        setTimeout(() => { btn.textContent = label; }, 1500);
      }).catch(() => {});
    });
  }
})();
