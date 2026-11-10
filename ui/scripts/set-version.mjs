// Keep the app manifests and lockfiles on the same version for locked builds.
// Usage (from app/):  npm --prefix ui run version:set -- 0.2.0
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error("Give a version like 0.2.0");
  process.exit(1);
}
const app = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const json = (file, edit) => {
  const p = join(app, file);
  const data = JSON.parse(readFileSync(p, "utf8"));
  edit(data);
  writeFileSync(p, JSON.stringify(data, null, 2) + "\n");
};
json("tauri.conf.json", (c) => (c.version = version));
json("ui/package.json", (c) => (c.version = version));
json("ui/package-lock.json", (c) => {
  c.version = version;
  c.packages[""].version = version;
});

const cargo = join(app, "Cargo.toml");
const text = readFileSync(cargo, "utf8");
// Only the [package] version (the first `version =` line in the file).
writeFileSync(cargo, text.replace(/^version = ".*"$/m, `version = "${version}"`));

const lock = join(app, "Cargo.lock");
const lockText = readFileSync(lock, "utf8");
const rootPackage = /(^\[\[package\]\]\r?\nname = "gitcraft"\r?\nversion = ")[^"\r\n]+/m;
if (!rootPackage.test(lockText)) throw new Error("Missing GitCraft package in Cargo.lock");
writeFileSync(lock, lockText.replace(rootPackage, (_, start) => start + version));

console.log(`Version set to ${version}.`);
