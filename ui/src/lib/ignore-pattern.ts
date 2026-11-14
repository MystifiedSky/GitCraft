/** Escape literal names; extension actions deliberately keep their wildcard. */
export function literalIgnorePath(path: string) {
  return "/" + path.replace(/[\\*?\[\]#! ]/g, character => "\\" + character);
}
