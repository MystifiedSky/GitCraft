type Key = { id: string; uid: string; fingerprint?: string };
function normalized(value: string) { return value.replace(/^0x/i, "").replace(/!$/, "").replaceAll(" ", "").toUpperCase(); }
export function signingOptions(keys: Key[], configured: string): [string,string][] {
  const value = normalized(configured);
  const matches = keys.filter(key => value && [key.id, key.fingerprint ?? ""].some(id => normalized(id) === value));
  const options: [string,string][] = [["", "(none)"], ...keys.map(key => [matches.length === 1 && key === matches[0] ? configured : key.fingerprint || key.id, `${key.id} - ${key.uid}`] as [string,string])];
  if (configured && !matches.length) options.push([configured, `${configured} (configured key not found)`]);
  return options;
}
