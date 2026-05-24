import { describe, expect, test } from 'bun:test';
import { classifyDownloadError, getProgressUpdate, selectDvidsFileUrl } from './downloadWorkerCore';

describe('download worker core', () => {
	test('classifies retryable and permanent failures', () => {
		expect(classifyDownloadError('HTTP 404: Not Found')).toBe('not_found');
		expect(classifyDownloadError('HTTP 403: Forbidden')).toBe('blocked');
		expect(classifyDownloadError('The operation was aborted')).toBe('cancelled');
		expect(classifyDownloadError('network timeout while fetching')).toBe('timeout');
		expect(classifyDownloadError('offset mismatch at byte 1024')).toBe('corrupt');
		expect(classifyDownloadError('No space left on device')).toBe('disk');
		expect(classifyDownloadError('connection reset')).toBe('network');
	});

	test('throttles progress by elapsed time or meaningful byte delta', () => {
		expect(
			getProgressUpdate({
				nowMs: 100,
				lastEmitMs: 0,
				bytesDownloaded: 128 * 1024,
				lastEmitBytes: 0
			})
		).toBe(false);

		expect(
			getProgressUpdate({
				nowMs: 600,
				lastEmitMs: 0,
				bytesDownloaded: 128 * 1024,
				lastEmitBytes: 0
			})
		).toBe(true);

		expect(
			getProgressUpdate({
				nowMs: 100,
				lastEmitMs: 0,
				bytesDownloaded: 2 * 1024 * 1024,
				lastEmitBytes: 0
			})
		).toBe(true);
	});

	test('selects the largest DVIDS media URL from nested metadata', () => {
		const payload = {
			results: [
				{ type: 'image/jpeg', src: 'https://media.example/thumb.jpg', width: 720 },
				{
					files: [
						{ type: 'video/mp4', url: 'https://media.example/small.mp4', filesize: 100 },
						{ type: 'video/mp4', url: 'https://media.example/large.mp4', filesize: 900 }
					]
				}
			]
		};

		expect(selectDvidsFileUrl(payload)).toBe('https://media.example/large.mp4');
	});
});
