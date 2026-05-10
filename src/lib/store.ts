import { writable } from 'svelte/store';

export type ViewMode = 'grid' | 'cards' | 'map' | 'link-analysis' | 'settings' | 'intelligence' | 'agent';

export const activeView = writable<ViewMode>('grid');
export const globalSearchOpen = writable<boolean>(false);
