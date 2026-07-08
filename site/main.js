// OS-aware download button + latest version from the GitHub Releases API.
const REPO = "duckyquang/Donna";

function detectOS() {
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
    windows: find(/set(up)?\.exe$/i) || find(/\.msi$/) || find(/\.exe$/),
    linux: find(/\.AppImage$/) || find(/\.deb$/),
  };
}

(async () => {
  const os = detectOS();
  const btn = document.querySelector("[data-download]");
  btn.textContent = OS_LABEL[os];
  const assets = await latestAssets().catch(() => null);
  if (!assets) return; // button already links to the releases page
  if (assets[os]) btn.href = assets[os];
  const v = document.querySelector("[data-version]");
  if (v && assets.version) v.textContent = `· ${assets.version}`;
})();
