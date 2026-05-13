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
