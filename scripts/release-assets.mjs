import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [version, repo] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+$/.test(version ?? "") || !/^[\w.-]+\/[\w.-]+$/.test(repo ?? "")) {
  throw new Error("Usage: node scripts/release-assets.mjs <version> <owner/repo>");
}

const output = "release-assets";
mkdirSync(output);
const platforms = {};
const uploaded = new Set();

function bundle(platform, suffix, targets = []) {
  const root = join("bundles", `release-${platform}`);
  const files = readdirSync(root, { recursive: true, withFileTypes: true });
  const matches = files.filter((file) => file.isFile() && file.name.endsWith(suffix));
  if (matches.length !== 1) {
    throw new Error(`Expected one ${platform} bundle ending with ${suffix}, found ${matches.length}`);
  }
  const { name, parentPath } = matches[0];
  copy(join(parentPath, name), name);
  if (targets.length) {
    const sigName = `${name}.sig`;
    const sigPath = join(parentPath, sigName);
    const signature = readFileSync(sigPath, "utf8").trim();
    if (!/^[A-Za-z0-9+/]+={0,2}$/.test(signature)) {
      throw new Error(`Missing or malformed updater signature: ${sigName}`);
    }
    copy(sigPath, sigName);
    const url = `https://github.com/${repo}/releases/download/v${version}/${encodeURIComponent(name)}`;
    for (const target of targets) platforms[target] = { url, signature };
  }
}

function copy(source, name) {
  if (uploaded.has(name)) throw new Error(`Duplicate release asset: ${name}`);
  copyFileSync(source, join(output, name));
  uploaded.add(name);
}

bundle("windows", "-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]);
bundle("windows", ".msi", ["windows-x86_64-msi"]);
bundle("macos", ".dmg");
bundle("macos", ".app.tar.gz", ["darwin-aarch64", "darwin-x86_64"]);
bundle("linux", ".AppImage", ["linux-x86_64"]);
bundle("linux", ".deb");
bundle("linux", ".rpm");

writeFileSync(join(output, "latest.json"), JSON.stringify({
  version,
  notes: `Release notes: https://github.com/${repo}/releases/tag/v${version}`,
  pub_date: new Date().toISOString(),
  platforms,
}, null, 2) + "\n");
