/**
 * A polling loop that never lets its task overlap with itself.
 *
 * The naive shape — calling an async task from a `requestAnimationFrame` loop
 * without awaiting it — issues a fresh request every frame regardless of
 * whether the previous one finished. As soon as the task takes longer than the
 * frame budget, requests queue without bound and the UI thread saturates. In
 * this app that happened the moment the signal generator started: the audio
 * callback began real FFT work under the same lock the poll needs, each poll
 * crossed 16.7 ms, and the window froze.
 *
 * Here the next tick is scheduled only after the current task settles, so at
 * most one is ever in flight. Under load the loop degrades to a lower poll
 * rate instead of collapsing.
 */
export const createPollLoop = <H>(
	task: () => Promise<void>,
	schedule: (callback: () => void) => H,
	cancel: (handle: H) => void,
): (() => void) => {
	let stopped = false;
	let handle: H | null = null;

	const tick = () => {
		if (stopped) {
			return;
		}
		// Rejections must not kill the loop: a single failed poll is transient,
		// and a dead loop leaves the display frozen on stale data.
		task()
			.catch(() => undefined)
			.finally(() => {
				if (!stopped) {
					handle = schedule(tick);
				}
			});
	};

	handle = schedule(tick);

	return () => {
		stopped = true;
		if (handle !== null) {
			cancel(handle);
			handle = null;
		}
	};
};
