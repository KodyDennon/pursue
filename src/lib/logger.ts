import { writable, get } from 'svelte/store';

// We can initialize this from an environment variable or local storage
const isDebugEnabled = writable(false);

export const logger = {
    debug: (...args: unknown[]) => {
        if (get(isDebugEnabled)) {
            console.debug(...args);
        }
    },
    info: (...args: unknown[]) => {
        console.log(...args);
    },
    error: (...args: unknown[]) => {
        console.error(...args);
    },
    enable: () => isDebugEnabled.set(true),
    disable: () => isDebugEnabled.set(false)
};
