import { describe, expect, test } from 'bun:test';
import {
	buildResolveDvidsMetadataArgs,
	classifyDownloadError,
	getDownloadDriver,
	getProgressUpdate,
	selectDvidsFileUrl
} from './downloadWorkerCore';

describe('download worker core', () => {
	test('classifies retryable and permanent failures', () => {
		expect(classifyDownloadError('HTTP 404: Not Found')).toBe('not_found');
		expect(classifyDownloadError('HTTP 403: Forbidden')).toBe('blocked');
		expect(classifyDownloadError('The operation was aborted')).toBe('cancelled');
		expect(classifyDownloadError('network timeout while fetching')).toBe('timeout');
		expect(classifyDownloadError('offset mismatch at byte 1024')).toBe('corrupt');
		expect(classifyDownloadError('No space left on device')).toBe('disk');
		expect(classifyDownloadError('connection reset')).toBe('network');
		expect(classifyDownloadError('Load failed')).toBe('network');
	});

	test('builds the Tauri camelCase payload for DVIDS metadata resolution', () => {
		expect(buildResolveDvidsMetadataArgs('1007879').videoId).toBe('1007879');
	});

	test('routes WAR.gov medialink assets to the WAR.gov-origin webview downloader', () => {
		const discoveredWarGovUrls = [
			new URL('/medialink/ufo/discovered-release/discovered-file.pdf', 'https://www.war.gov').href,
			new URL('/Portals/1/Interactive/2099/UFO/discovered-data.csv', 'https://www.war.gov').href
		];

		for (const url of discoveredWarGovUrls) {
			expect(getDownloadDriver(url)).toBe('war-gov-webview');
		}
	});

	test('keeps CloudFront assets on the normal browser worker path', () => {
		expect(getDownloadDriver('https://discovered-distribution.cloudfront.net/discovered.zip')).toBe(
			'browser-worker'
		);
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
