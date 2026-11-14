// Notification-center rules: unread badge, date lines, status-bar hint.

/** getUnreadNotificationCountString: the count, "50+" from 50 on; empty when there is nothing unread (no badge). */
export function unreadBadge(unread: number): string {
  if (unread <= 0) return "";
  return unread >= 50 ? "50+" : String(unread);
}

const ordinal = (n: number) => {
  const rem = n % 100;
  if (rem >= 11 && rem <= 13) return `${n}th`;
  return `${n}${({ 1: "st", 2: "nd", 3: "rd" } as Record<number, string>)[n % 10] ?? "th"}`;
};

/**
 * Date line of a notification row: "Today" from midnight on, "Yesterday" for the day before, otherwise
 * "MMMM Do" (NOTIFICATION_ELAPSED_TIME, English); then " @ " and the time as hh:mm, lower-cased, space removed.
 */
export function formatNoticeDate(created: Date, now: Date = new Date()): string {
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0).getTime();
  const startOfYesterday = startOfToday - 864e5;
  const t = created.getTime();
  const day = t >= startOfToday ? "Today" : t >= startOfYesterday ? "Yesterday" : `${created.toLocaleDateString("en-US", { month: "long" })} ${ordinal(created.getDate())}`;
  const time = created.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }).toLowerCase().replace(" ", "");
  return `${day} @ ${time}`;
}

/** A pull request as far as the status bar needs it. */
export interface StatusPr {
  number: number;
  repo: string;
  draft: boolean;
}

/**
 * Status-bar buckets, in display order. Their real order depends on a per-bucket priority
 * that GitCraft cannot know, so this order is an estimate.
 * Buckets pinned, snoozed and other are never shown.
 */
export const STATUS_BUCKETS: { id: string; sentence: string }[] = [
  { id: "readyToMerge", sentence: "is ready to merge" },
  { id: "unassignedReviewers", sentence: "has no assigned reviewers" },
  { id: "failingCI", sentence: "is failing CI" },
  { id: "conflicts", sentence: "has merge conflicts" },
  { id: "needsMyReview", sentence: "needs your review" },
  { id: "changesRequested", sentence: "has suggested changes" },
  { id: "reviewerCommented", sentence: "has comments" },
  { id: "waitingForReview", sentence: "is waiting on a review" },
  { id: "draft", sentence: "is in draft" },
];

/** GitCraft only knows whether a pull request is a draft; every other open one falls in the hidden "other" bucket. */
export function bucketOf(pr: StatusPr): string {
  return pr.draft ? "draft" : "other";
}

/** "{owner}/{repo}#{n} is in draft": the sentence of the first shown bucket's first pull request, or null for none. */
export function pullRequestSentence(prs: StatusPr[]): string | null {
  for (const b of STATUS_BUCKETS) {
    const pr = prs.find((p) => bucketOf(p) === b.id);
    if (pr) return `${pr.repo}#${pr.number} ${b.sentence}`;
  }
  return null;
}

/**
 * The status bar's integration hint: nothing connected -> "Connect an integration";
 * something connected but not this repository's service -> "Select an integration"; this repository's service connected ->
 * the first bucket's pull request sentence, or "No Pull Requests" when no shown bucket has one. `prs` is null while loading.
 */
export function integrationHint(connectedIds: string[], repoService: string | null | undefined, prs: StatusPr[] | null): string | null {
  if (!connectedIds.length) return "Connect an integration";
  if (!repoService || !connectedIds.includes(repoService)) return "Select an integration";
  if (prs === null) return null;
  return pullRequestSentence(prs) ?? "No Pull Requests";
}
