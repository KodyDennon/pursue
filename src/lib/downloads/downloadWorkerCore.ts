export type DownloadErrorClass =
	| 'blocked'
	| 'not_found'
	| 'cancelled'
	| 'timeout'
	| 'corrupt'
	| 'disk'
	| 'network'
	| 'unknown';

export interface ProgressUpdateInput {
	nowMs: number;
	lastEmitMs: number;
	bytesDownloaded: number;
	lastEmitBytes: number;
	minIntervalMs?: number;
	minByteDelta?: number;
}

export type DownloadDriver = 'browser-worker' | 'war-gov-webview';

const BINARY_STRING_CHUNK_BYTES = 32 * 1024;

export function classifyDownloadError(error: unknown): DownloadErrorClass {
	const message = String(error instanceof Error ? error.message : error).toLowerCase();

	if (message.includes('404') || message.includes('not found')) return 'not_found';
	if (message.includes('403') || message.includes('forbidden') || message.includes('blocked')) {
		return 'blocked';
	}
	if (message.includes('abort') || message.includes('cancel')) return 'cancelled';
	if (message.includes('timeout') || message.includes('timed out')) return 'timeout';
	if (message.includes('offset') || message.includes('hash') || message.includes('corrupt')) {
		return 'corrupt';
	}
	if (
		message.includes('no space') ||
		message.includes('disk') ||
		message.includes('quota') ||
		message.includes('enospc')
	) {
		return 'disk';
	}
	if (
		message.includes('network') ||
		message.includes('connection') ||
		message.includes('fetch failed') ||
		message.includes('failed to fetch') ||
		message.includes('load failed')
	) {
		return 'network';
	}
	return 'unknown';
}

export function buildResolveDvidsMetadataArgs(assetId: string): { videoId: string } {
	return { videoId: assetId };
}

export function bytesToBase64(bytes: Uint8Array): string {
	let binary = '';
	for (let offset = 0; offset < bytes.byteLength; offset += BINARY_STRING_CHUNK_BYTES) {
		const slice = bytes.subarray(offset, offset + BINARY_STRING_CHUNK_BYTES);
		binary += String.fromCharCode(...slice);
	}
	return btoa(binary);
}

export function getDownloadDriver(rawUrl: string): DownloadDriver {
	const url = new URL(rawUrl);
	const host = url.hostname.toLowerCase();
	return host === 'war.gov' || host === 'www.war.gov' ? 'war-gov-webview' : 'browser-worker';
}

export function getProgressUpdate(input: ProgressUpdateInput): boolean {
	const minIntervalMs = input.minIntervalMs ?? 500;
	const minByteDelta = input.minByteDelta ?? 1024 * 1024;
	return (
		input.nowMs - input.lastEmitMs >= minIntervalMs ||
		input.bytesDownloaded - input.lastEmitBytes >= minByteDelta
	);
}

export function selectDvidsFileUrl(payload: unknown): string | null {
	const candidates: Array<{ score: number; url: string }> = [];
	collectDvidsCandidates(payload, candidates);
	candidates.sort((a, b) => b.score - a.score);
	return candidates[0]?.url ?? null;
}

function collectDvidsCandidates(value: unknown, candidates: Array<{ score: number; url: string }>) {
	if (Array.isArray(value)) {
		for (const item of value) collectDvidsCandidates(item, candidates);
		return;
	}

	if (!value || typeof value !== 'object') return;
	const map = value as Record<string, unknown>;
	const url = ['src', 'url', 'download_url', 'file']
		.map((key) => map[key])
		.find((candidate): candidate is string => {
			return (
				typeof candidate === 'string' &&
				(candidate.startsWith('https://') || candidate.startsWith('http://'))
			);
		});

	if (url) {
		const mediaType = String(map.type ?? map.mime_type ?? map.mime ?? '').toLowerCase();
		const size =
			numericValue(map.filesize) ?? numericValue(map.file_size) ?? numericValue(map.size) ?? 0;
		const width = numericValue(map.width) ?? 0;
		const mediaScore = mediaType.includes('video') ? 1_000_000_000 : 0;
		candidates.push({ score: mediaScore + size + width, url });
	}

	for (const child of Object.values(map)) collectDvidsCandidates(child, candidates);
}

function numericValue(value: unknown): number | null {
	if (typeof value === 'number' && Number.isFinite(value)) return value;
	if (typeof value === 'string') {
		const parsed = Number.parseInt(value, 10);
		if (Number.isFinite(parsed)) return parsed;
	}
	return null;
}
