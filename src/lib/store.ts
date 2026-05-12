import { writable } from 'svelte/store';

export type ViewMode =
	| 'dashboard'
	| 'map'
	| 'link-analysis'
	| 'intelligence'
	| 'agent'
	| 'vault'
	| 'settings';

export const activeView = writable<ViewMode>('dashboard');

export const globalSearchOpen = writable<boolean>(false);
