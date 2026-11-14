/** Binds asynchronous work to the inputs and edits that existed when it began. */
export class RevisionGuard {
  private key = "";
  private revision = 0;
  observe(key: string) {
    if (key !== this.key) { this.key = key; this.invalidate(); }
  }
  invalidate() { this.revision++; }
  capture() {
    const revision = this.revision;
    return () => revision === this.revision;
  }
}
