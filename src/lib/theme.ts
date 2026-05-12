import { writable } from 'svelte/store';

export type ThemePalette = {
	name: string;
	background: string;
	surface: string;
	surfaceElevated: string;
	border: string;
	textPrimary: string;
	textSecondary: string;
	accentPrimary: string;
	accentSuccess: string;
	accentDanger: string;
};

export const defaultBlackOpsTheme: ThemePalette = {
	name: 'Black-Ops Default',
	background: '#0a0b0d',
	surface: '#101114',
	surfaceElevated: 'rgba(20, 22, 26, 0.6)',
	border: 'rgba(255, 255, 255, 0.08)',
	textPrimary: '#f5f6f8',
	textSecondary: '#8a8f98',
	accentPrimary: '#e7c46b', // Gold
	accentSuccess: '#4df3a9', // Emerald
	accentDanger: '#f34d4d' // Crimson
};

// You can add more themes here like a 'Terminal Green' theme

function createThemeStore() {
	const isBrowser = typeof window !== 'undefined';
	const stored = isBrowser ? localStorage.getItem('pursue-theme') : null;
	let initial = defaultBlackOpsTheme;

	if (stored) {
		try {
			initial = JSON.parse(stored);
		} catch (e) {
			console.error('Failed to parse stored theme', e);
		}
	}

	const { subscribe, set } = writable<ThemePalette>(initial);

	if (isBrowser) {
		subscribe((theme) => {
			applyThemeToDocument(theme);
		});
	}

	return {
		subscribe,
		set: (theme: ThemePalette) => {
			if (isBrowser) {
				localStorage.setItem('pursue-theme', JSON.stringify(theme));
			}
			set(theme);
		},
		reset: () => {
			if (isBrowser) {
				localStorage.removeItem('pursue-theme');
			}
			set(defaultBlackOpsTheme);
		}
	};
}

export const activeTheme = createThemeStore();

export function applyThemeToDocument(theme: ThemePalette) {
	if (typeof document === 'undefined') return;
	const root = document.documentElement;
	root.style.setProperty('--bg-base', theme.background);
	root.style.setProperty('--bg-surface', theme.surface);
	root.style.setProperty('--bg-surface-elevated', theme.surfaceElevated);
	root.style.setProperty('--border-subtle', theme.border);
	root.style.setProperty('--text-primary', theme.textPrimary);
	root.style.setProperty('--text-secondary', theme.textSecondary);
	root.style.setProperty('--accent-primary', theme.accentPrimary);
	root.style.setProperty('--accent-success', theme.accentSuccess);
	root.style.setProperty('--accent-danger', theme.accentDanger);
}
