import { writable } from 'svelte/store';

export type ViewMode = 'grid' | 'cards' | 'map' | 'link-analysis' | 'settings';

export const activeView = writable<ViewMode>('grid');
export const globalSearchOpen = writable<boolean>(false);
