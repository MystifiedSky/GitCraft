import { describe, expect, it } from "vitest";
import type { GitRef } from "./api";
import { groupCommitRefs } from "./commit-refs";
const ref = (name: string, kind: GitRef["kind"], upstream = "", sha = "same"): GitRef => ({ name, kind, upstream, sha, full: `${kind}/${name}`, ahead: 0, behind: 0 });
describe("commit reference grouping", () => {
  it("counts two local/remote branch pairs as two labels", () => {
    const skylar = ref("Skylar", "Local", "origin/Skylar");
    const main = ref("main", "Local", "origin/main");
    const remoteSkylar = ref("origin/Skylar", "Remote");
    const remoteMain = ref("origin/main", "Remote");
    expect(groupCommitRefs([skylar, main, remoteSkylar, remoteMain])).toEqual([
      { ref: skylar, remote: remoteSkylar }, { ref: main, remote: remoteMain },
    ]);
  });
  it("uses the configured upstream even when its branch name differs", () => {
    const local = ref("feature", "Local", "refs/remotes/team/review");
    const remote = ref("team/review", "Remote");
    expect(groupCommitRefs([local, remote])).toEqual([{ ref: local, remote }]);
  });
  it("keeps diverged refs, unrelated branches and tags separate", () => {
    const refs = [ref("main", "Local", "origin/main"), ref("origin/main", "Remote", "", "different"), ref("v1", "Tag")];
    expect(groupCommitRefs(refs)).toHaveLength(3);
  });
  it("groups an unconfigured unique match but does not guess between remotes or nested branch names", () => {
    const local = ref("feature/topic", "Local");
    const remote = ref("origin/feature/topic", "Remote");
    expect(groupCommitRefs([local, remote])).toEqual([{ ref: local, remote }]);
    expect(groupCommitRefs([local, remote, ref("other/feature/topic", "Remote")])).toHaveLength(3);
    expect(groupCommitRefs([ref("main", "Local"), ref("origin/team/main", "Remote")])).toHaveLength(2);
  });
});
