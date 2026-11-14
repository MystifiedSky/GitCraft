/** A shared bound for independent repository reads; queued requests honor cancellation. */
export function createReadPool(limit: number) {
  let active = 0;
  const queue: (() => void)[] = [];
  const drain = () => { while (active < limit && queue.length) queue.shift()!(); };
  return function run<T>(work: () => Promise<T>, signal?: AbortSignal): Promise<T> {
    return new Promise((resolve,reject) => {
      const start = () => {
        if (signal?.aborted) { reject(new DOMException("Read cancelled", "AbortError")); return; }
        active++;
        void Promise.resolve().then(work).then(resolve,reject).finally(() => { active--; drain(); });
      };
      queue.push(start); drain();
    });
  };
}
export const readRepositorySummary = createReadPool(4);
