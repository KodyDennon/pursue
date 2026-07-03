// Plain module-level flag, not a Svelte store: nothing subscribes to this reactively, it's
// only ever read imperatively inside debug() below, so svelte/store added nothing here.
let isDebugEnabled = false;

export const logger = {
	debug: (...args: unknown[]) => {
		if (isDebugEnabled) {
			console.debug(...args);
		}
	},
	info: (...args: unknown[]) => {
		console.log(...args);
	},
	error: (...args: unknown[]) => {
		console.error(...args);
	},
	enable: () => {
		isDebugEnabled = true;
	},
	disable: () => {
		isDebugEnabled = false;
	}
};
