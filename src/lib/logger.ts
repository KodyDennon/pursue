import { writable, get } from 'svelte/store';

// We can initialize this from an environment variable or local storage
const isDebugEnabled = writable(false);

// If we are in development mode, default to true or check for a flag
if (import.meta.env.DEV) {
    isDebugEnabled.set(true);
}

export const logger = {
    debug: (...args: unknown[]) => {
        if (get(isDebugEnabled)) {
            console.warn(...args);
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
