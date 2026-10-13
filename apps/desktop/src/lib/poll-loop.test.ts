import { describe, expect, it } from "bun:test";
import { createPollLoop } from "./poll-loop";

/** Resolve after the microtask queue drains, so pending promises settle. */
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

describe("createPollLoop", () => {
	it("never runs the task concurrently, even when it is slower than the tick", async () => {
		let inFlight = 0;
		let maxInFlight = 0;
		let completed = 0;

		// A task slower than the scheduler: this is the freeze scenario.
		const task = async () => {
			inFlight += 1;
			maxInFlight = Math.max(maxInFlight, inFlight);
			await new Promise((resolve) => setTimeout(resolve, 5));
			inFlight -= 1;
			completed += 1;
		};

		// A scheduler that fires far faster than the task can complete.
		const timers: ReturnType<typeof setTimeout>[] = [];
		const schedule = (cb: () => void) => {
			const id = setTimeout(cb, 1);
			timers.push(id);
			return id;
		};
		const cancel = (handle: ReturnType<typeof setTimeout>) => clearTimeout(handle);

		const stop = createPollLoop(task, schedule, cancel);
		await new Promise((resolve) => setTimeout(resolve, 60));
		stop();

		expect(maxInFlight).toBe(1);
		expect(completed).toBeGreaterThan(0);
	});

	it("keeps polling after the task rejects", async () => {
		let calls = 0;
		const task = async () => {
			calls += 1;
			throw new Error("backend unavailable");
		};

		const schedule = (cb: () => void) => setTimeout(cb, 1);
		const cancel = (handle: ReturnType<typeof setTimeout>) => clearTimeout(handle);

		const stop = createPollLoop(task, schedule, cancel);
		await new Promise((resolve) => setTimeout(resolve, 30));
		stop();

		expect(calls).toBeGreaterThan(1);
	});

	it("stops scheduling once stopped, and runs no further tasks", async () => {
		let calls = 0;
		const task = async () => {
			calls += 1;
		};

		const schedule = (cb: () => void) => setTimeout(cb, 1);
		const cancel = (handle: ReturnType<typeof setTimeout>) => clearTimeout(handle);

		const stop = createPollLoop(task, schedule, cancel);
		await new Promise((resolve) => setTimeout(resolve, 15));
		stop();

		const callsAtStop = calls;
		await new Promise((resolve) => setTimeout(resolve, 30));
		expect(calls).toBe(callsAtStop);
	});

	it("does not start a new task after stop, even if one was already in flight", async () => {
		let started = 0;
		const task = async () => {
			started += 1;
			await new Promise((resolve) => setTimeout(resolve, 20));
		};

		const schedule = (cb: () => void) => setTimeout(cb, 1);
		const cancel = (handle: ReturnType<typeof setTimeout>) => clearTimeout(handle);

		const stop = createPollLoop(task, schedule, cancel);
		await flush();
		await new Promise((resolve) => setTimeout(resolve, 5));
		stop();

		await new Promise((resolve) => setTimeout(resolve, 60));
		expect(started).toBe(1);
	});
});
