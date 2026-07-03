import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * Formats a byte value into a human-readable string.
 */
export function formatBytes(value: number | null | undefined): string {
	if (!value || isNaN(value)) return '0 B';
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let next = value;
	let unit = 0;
	while (next >= 1024 && unit < units.length - 1) {
		next /= 1024;
		unit += 1;
	}
	return `${next.toFixed(next >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

/**
 * Resolves a record's library-relative asset path (thumbnail, artifact) to a src the webview
 * can load, given the app's library root directory.
 */
export function resolveLibraryAssetPath(
	libraryPath: string | null | undefined,
	relativePath: string | null | undefined
): string {
	if (!relativePath || !libraryPath) return '';
	const cleanLib =
		libraryPath.endsWith('/') || libraryPath.endsWith('\\') ? libraryPath : libraryPath + '/';
	return convertFileSrc(cleanLib + relativePath);
}
