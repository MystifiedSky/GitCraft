import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseEntries, recordRelease, releaseNotes } from './changelog-release.mjs';

const shipped = `# Changelog

## [Unreleased]

### Added

- First feature.

- Second feature,
  continued on a second line.

### Fixed

- A fix.

## [0.1.5] - 2026-11-11

### Added

- Old.

[Unreleased]: https://github.com/o/r/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/o/r/compare/v0.1.4...v0.1.5
`;

// A later commit added one more entry and reworded nothing.
const current = shipped.replace('### Fixed', '- Third feature, added after the release.\n\n### Fixed');

test('parses bullets with continuation lines and ignores prose', () => {
  const entries = parseEntries('Expected next release: `0.1.6`.\n\n### Added\n\n- One.\n\n- Two\n  more.\n');
  assert.deepEqual(entries, { Added: ['- One.', '- Two\n  more.'] });
});

test('moves only the shipped entries into a dated section and updates the links', () => {
  const out = recordRelease({ current, shipped, version: '0.1.6', repo: 'o/r', date: '2026-11-12' });
  const [unreleased, rest] = out.split('## [0.1.6] - 2026-11-12');
  assert.match(unreleased, /Third feature/);
  assert.doesNotMatch(unreleased, /First feature/);
  assert.match(rest, /First feature/);
  assert.match(rest, /continued on a second line/);
  assert.match(rest, /A fix/);
  assert.doesNotMatch(rest.split('## [0.1.5]')[0], /Third feature/);
  assert.match(out, /\[Unreleased\]: https:\/\/github.com\/o\/r\/compare\/v0\.1\.6\.\.\.HEAD/);
  assert.match(out, /\[0\.1\.6\]: https:\/\/github.com\/o\/r\/compare\/v0\.1\.5\.\.\.v0\.1\.6/);
  assert.match(out, /\[0\.1\.5\]: /);
});

test('is idempotent and ignores releases with no entries', () => {
  const once = recordRelease({ current, shipped, version: '0.1.6', repo: 'o/r', date: '2026-11-12' });
  assert.equal(recordRelease({ current: once, shipped, version: '0.1.6', repo: 'o/r', date: '2026-11-12' }), null);
  const empty = shipped.replace(/### Added[\s\S]*?(?=## \[0\.1\.5\])/, '');
  assert.equal(recordRelease({ current: empty, shipped: empty, version: '0.1.7', repo: 'o/r', date: '2026-11-12' }), null);
});

test('release notes list the Unreleased entries by category', () => {
  const notes = releaseNotes(shipped);
  assert.match(notes, /^### Added\n\n- First feature\./);
  assert.match(notes, /### Fixed\n\n- A fix\./);
  assert.equal(releaseNotes('# C\n\n## [Unreleased]\n\n## [0.1.0] - 2026-01-01\n'), '');
});

test('category headings and trailing whitespace accept long inputs without backtracking', () => {
  assert.deepEqual(parseEntries(`### Fixed${' '.repeat(100000)}\n\n- Kept.`), { Fixed: ['- Kept.'] });
  const current = `# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n- Kept.\n\n[old]: https://example.test/old\n${' '.repeat(100000)}`;
  const released = recordRelease({ current, shipped: current, version: '1.0.0', repo: 'owner/project', date: '2026-11-14' });
  assert.ok(released.includes('[Unreleased]: https://github.com/owner/project/compare/v1.0.0...HEAD'));
});

test('release versions are validated instead of interpreted as regular expressions', () => {
  for (const version of ['1.2.3\\', '1.2.3.*', '(a+)+$', '1.2.3]\n## [Unreleased]', '', undefined]) {
    assert.throws(() => recordRelease({ current, shipped, version, repo: 'o/r', date: '2026-11-14' }), /Release version/);
  }
  const withNearbyVersion = current + '\n## [10.1.6] - 2026-11-13\n';
  assert.ok(recordRelease({ current: withNearbyVersion, shipped, version: '0.1.6', repo: 'o/r', date: '2026-11-14' }));
});
