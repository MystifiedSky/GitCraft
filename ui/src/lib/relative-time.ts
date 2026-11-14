// "a few seconds ago", "5 minutes ago", "a day ago": relative time in English with the usual rounding thresholds
// (45 s, 45 min, 22 h, 26 d, 11 months). Each unit is rounded on its own, so 89 minutes is still "an hour".
const THRESHOLDS = { seconds: 44, minutes: 45, hours: 22, days: 26, months: 11 } as const;

/** Relative wording of `then` seen from `now`: "… ago" for the past, "in …" for the future. */
export function fromNow(then: number, now: number = Date.now()): string {
  const diff = now - then;
  const ms = Math.abs(diff);
  const seconds = Math.round(ms / 1000);
  const minutes = Math.round(ms / 6e4);
  const hours = Math.round(ms / 36e5);
  const days = Math.round(ms / 864e5);
  const months = Math.round(((ms / 864e5) * 4800) / 146097);
  const years = Math.round(((ms / 864e5) * 4800) / 146097 / 12);
  let text: string;
  if (seconds <= THRESHOLDS.seconds) text = "a few seconds";
  else if (minutes <= 1) text = "a minute";
  else if (minutes < THRESHOLDS.minutes) text = `${minutes} minutes`;
  else if (hours <= 1) text = "an hour";
  else if (hours < THRESHOLDS.hours) text = `${hours} hours`;
  else if (days <= 1) text = "a day";
  else if (days < THRESHOLDS.days) text = `${days} days`;
  else if (months <= 1) text = "a month";
  else if (months < THRESHOLDS.months) text = `${months} months`;
  else if (years <= 1) text = "a year";
  else text = `${years} years`;
  return diff >= 0 ? `${text} ago` : `in ${text}`;
}
