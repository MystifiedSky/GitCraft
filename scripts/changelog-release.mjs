// Keeps CHANGELOG.md in step with published releases.
//
//   node scripts/changelog-release.mjs notes  <changelog>                          print the Unreleased entries as release notes
//   node scripts/changelog-release.mjs record <version> <owner/repo> <shipped-changelog> [--date YYYY-MM-DD]
//                                                                                   move the entries that shipped into a dated section
//
// "shipped-changelog" is CHANGELOG.md as it was in the released commit. Only entries found there are moved, so entries added to
// Unreleased by commits pushed after the release stay under Unreleased. Running it twice for one version changes nothing.
import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const CATEGORIES = ['Added', 'Changed', 'Deprecated', 'Removed', 'Fixed', 'Security'];

const normalize = (text) => text.replace(/\r\n/g, '\n');

/** Splits the file around the Unreleased section: text before it, its body, and everything after (versions and link references). */
export function splitUnreleased(text) {
  const src = normalize(text);
  const start = src.search(/^## \[Unreleased\][^\n]*$/m);
  if (start < 0) throw new Error('CHANGELOG.md has no "## [Unreleased]" section.');
  const afterHeading = src.indexOf('\n', start) + 1;
  const rest = src.slice(afterHeading);
  const next = rest.search(/^(## \[|\[[^\]]+\]: )/m);
  const bodyEnd = next < 0 ? rest.length : next;
  return { head: src.slice(0, afterHeading), body: rest.slice(0, bodyEnd), tail: rest.slice(bodyEnd) };
}

/** Reads a section body into { Added: [entry, ...], ... }. An entry is a bullet with its continuation lines, unchanged. */
export function parseEntries(body) {
  const out = {};
  let category = null;
  let entry = null;
  const flush = () => {
    if (category && entry) (out[category] ??= []).push(entry.join('\n').trimEnd());
    entry = null;
  };
  for (const line of normalize(body).split('\n')) {
    const heading = /^### (.+?)\s*$/.exec(line);
    if (heading) {
      flush();
      category = heading[1];
    } else if (/^- /.test(line)) {
      flush();
      entry = [line];
    } else if (entry && (line.trim() === '' || /^\s+/.test(line))) {
      entry.push(line);
    } else if (line.trim() !== '') {
      flush(); // prose such as an old "Expected next release" note: not an entry
    }
  }
  flush();
  return out;
}

const categoryOrder = (names) => [...CATEGORIES.filter((c) => names.includes(c)), ...names.filter((c) => !CATEGORIES.includes(c))];

/** Writes entries back as "### Category" blocks with a blank line between entries. */
export function renderEntries(entries) {
  const names = categoryOrder(Object.keys(entries).filter((c) => entries[c]?.length));
  return names.map((c) => `### ${c}\n\n${entries[c].join('\n\n')}\n`).join('\n');
}

export const isEmpty = (entries) => Object.values(entries).every((list) => !list?.length);

/** The shipped entries as GitHub release notes (empty string when there are none). */
export function releaseNotes(changelogText) {
  const entries = parseEntries(splitUnreleased(changelogText).body);
  return isEmpty(entries) ? '' : renderEntries(entries);
}

/**
 * Returns the new CHANGELOG.md text, or null when nothing needs to change (the version is already recorded, or nothing shipped).
 */
export function recordRelease({ current, shipped, version, repo, date }) {
  const text = normalize(current);
  if (new RegExp(`^## \\[${version.replace(/\./g, '\\.')}\\]`, 'm').test(text)) return null;
  const shippedEntries = parseEntries(splitUnreleased(shipped).body);
  if (isEmpty(shippedEntries)) return null;

  const { head, body, tail } = splitUnreleased(text);
  const remaining = parseEntries(body);
  const moved = {};
  for (const [category, list] of Object.entries(shippedEntries)) {
    for (const entry of list) {
      const at = remaining[category]?.indexOf(entry) ?? -1;
      if (at >= 0) {
        remaining[category].splice(at, 1);
        (moved[category] ??= []).push(entry);
      }
    }
  }
  // The shipped entries are the record of the release even if a later commit reworded one of them in Unreleased.
  const section = isEmpty(moved) ? shippedEntries : moved;

  const unreleased = isEmpty(remaining) ? '\n' : `\n${renderEntries(remaining)}\n`;
  let tailText = tail;
  const compare = /^\[Unreleased\]: (https:\/\/github\.com\/[^/]+\/[^/]+)\/compare\/(v[^.\s]+\.[^.\s]+\.[^.\s]+)\.\.\.HEAD\s*$/m.exec(tailText);
  const base = compare?.[1] ?? `https://github.com/${repo}`;
  const previous = compare?.[2];
  const links = [`[Unreleased]: ${base}/compare/v${version}...HEAD`, previous ? `[${version}]: ${base}/compare/${previous}...v${version}` : `[${version}]: ${base}/releases/tag/v${version}`];
  if (compare) tailText = tailText.replace(compare[0], links.join('\n'));
  else tailText = `${tailText.replace(/\s*$/, '\n')}\n${links.join('\n')}\n`;

  return `${head}${unreleased}\n## [${version}] - ${date}\n\n${renderEntries(section)}\n${tailText.replace(/^\n+/, '')}`.replace(/\n{3,}/g, '\n\n');
}

function main(argv) {
  const [command, ...args] = argv;
  if (command === 'notes') {
    process.stdout.write(releaseNotes(readFileSync(args[0], 'utf8')));
  } else if (command === 'record') {
    const [version, repo, shippedPath] = args;
    const flag = args.indexOf('--date');
    const date = flag >= 0 ? args[flag + 1] : new Date().toISOString().slice(0, 10);
    if (!/^\d+\.\d+\.\d+$/.test(version ?? '') || !repo || !shippedPath) throw new Error('Usage: record <version> <owner/repo> <shipped-changelog> [--date YYYY-MM-DD]');
    const current = readFileSync('CHANGELOG.md', 'utf8');
    const next = recordRelease({ current, shipped: readFileSync(shippedPath, 'utf8'), version, repo, date });
    if (next === null) console.log(`CHANGELOG.md already records ${version} or nothing shipped; no change.`);
    else {
      writeFileSync('CHANGELOG.md', next);
      console.log(`Recorded ${version} in CHANGELOG.md.`);
    }
  } else {
    throw new Error('Usage: changelog-release.mjs notes <changelog> | record <version> <owner/repo> <shipped-changelog> [--date YYYY-MM-DD]');
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main(process.argv.slice(2));
