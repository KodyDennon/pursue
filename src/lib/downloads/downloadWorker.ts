import {
	buildResolveDvidsMetadataArgs,
	bytesToBase64,
	classifyDownloadError,
	getDownloadDriver,
	getProgressUpdate,
	selectDvidsFileUrl,
	type DownloadErrorClass
} from './downloadWorkerCore';
import type { BulkDownloadItem } from '$lib/types';

const DEFAULT_CONCURRENCY = 3;
const MAX_CHUNK_BYTES = 256 * 1024; // Reduced chunk size for bridge stability
const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 2000;

type HostCommand =
	| 'begin_download_item'
	| 'append_download_chunk'
	| 'finalize_download_item'
	| 'fail_download_item'
	| 'reset_download_item_part'
	| 'resolve_dvids_metadata'
	| 'download_war_gov_item_with_webview';

interface StartMessage {
	type: 'start';
	jobId: string;
	items: BulkDownloadItem[];
	concurrency?: number;
}

interface CancelMessage {
	type: 'cancel';
	jobId?: string;
}

interface HostResultMessage {
	type: 'host-result';
	id: number;
	ok: boolean;
	value?: unknown;
	error?: string;
}

type IncomingMessage = StartMessage | CancelMessage | HostResultMessage;

interface HostCall {
	type: 'host-call';
	id: number;
	command: HostCommand;
	args: Record<string, unknown>;
}

interface WorkerProgress {
	type: 'progress';
	jobId: string;
	itemId: string;
	bytesDownloaded: number;
	totalBytes: number | null;
}

interface WorkerItemEvent {
	type: 'item-completed' | 'item-failed';
	jobId: string;
	itemId: string;
	error?: string;
	errorClass?: DownloadErrorClass;
}

interface WorkerIdle {
	type: 'idle';
	jobId: string;
}

type OutgoingMessage = HostCall | WorkerProgress | WorkerItemEvent | WorkerIdle;

interface BeginResponse {
	offset: number;
	cancel_requested: boolean;
}

interface AppendResponse {
	offset: number;
}

const pending = new Map<
	number,
	{ resolve: (value: unknown) => void; reject: (error: Error) => void }
>();
const controllers = new Map<string, AbortController>();
let nextCallId = 1;
let isRunning = false;

self.onmessage = (event: MessageEvent<IncomingMessage>) => {
	const message = event.data;
	if (message.type === 'host-result') {
		const waiter = pending.get(message.id);
		if (!waiter) return;
		pending.delete(message.id);
		if (message.ok) waiter.resolve(message.value);
		else waiter.reject(new Error(message.error || 'host command failed'));
		return;
	}

	if (message.type === 'cancel') {
		for (const [key, controller] of controllers) {
			if (!message.jobId || key.startsWith(`${message.jobId}:`)) {
				controller.abort();
			}
		}
		return;
	}

	if (message.type === 'start') {
		if (isRunning) return;
		void runQueue(message);
	}
};

async function runQueue(message: StartMessage) {
	isRunning = true;
	console.log(`[Worker] Starting job ${message.jobId} with ${message.items.length} items`);
	try {
		const concurrency = Math.min(4, Math.max(1, message.concurrency ?? DEFAULT_CONCURRENCY));
		const queue = [...message.items];
		const workers = Array.from({ length: concurrency }, async (v, i) => {
			while (queue.length > 0) {
				const item = queue.shift();
				if (!item) return;
				console.log(`[Worker ${i}] Shifting: ${item.title} (ID: ${item.id})`);
				await downloadItem(message.jobId, item);
			}
		});
		await Promise.all(workers);
	} catch (e) {
		console.error('[Worker] Fatal queue error:', e);
	} finally {
		isRunning = false;
		console.log(`[Worker] Job ${message.jobId} finished.`);
		post({ type: 'idle', jobId: message.jobId });
	}
}

async function downloadItem(jobId: string, item: BulkDownloadItem) {
	const controller = new AbortController();
	controllers.set(`${jobId}:${item.id}`, controller);
	try {
		if (!item.url) throw new Error('No source URL available');
		const source = await resolveSource(item.url, item.record_id, controller.signal);
		let begin = (await hostCall('begin_download_item', {
			request: {
				job_id: jobId,
				item_id: item.id,
				url: item.url,
				resolved_url: source.url,
				source_host: source.host
			}
		})) as BeginResponse;
		if (begin.cancel_requested) throw new DOMException('Download cancelled', 'AbortError');

		if (getDownloadDriver(source.url) === 'war-gov-webview') {
			await hostCall('download_war_gov_item_with_webview', {
				request: {
					job_id: jobId,
					item_id: item.id,
					record_id: item.record_id,
					url: item.url,
					resolved_url: source.url,
					content_type: item.content_type ?? null
				}
			});
			post({ type: 'item-completed', jobId, itemId: item.id });
			return;
		}

		let response: Response;
		response = await fetchWithResume(source.url, begin.offset, controller.signal);
		if (response.status === 416) {
			// Range not satisfiable - local part might be larger than remote file. Reset and retry.
			await hostCall('reset_download_item_part', { itemId: item.id });
			begin = { ...begin, offset: 0 };
			response = await fetchWithResume(source.url, 0, controller.signal);
		} else if (begin.offset > 0 && response.status === 200) {
			await hostCall('reset_download_item_part', { itemId: item.id });
			begin = { ...begin, offset: 0 };
		}
		if (!response.ok && response.status !== 206) {
			throw new Error(`HTTP ${response.status}: ${response.statusText}`);
		}

		const contentLength = response.headers.get('content-length');
		const remainingBytes = contentLength ? Number.parseInt(contentLength, 10) : null;
		const expectedSize =
			remainingBytes !== null && Number.isFinite(remainingBytes)
				? remainingBytes + begin.offset
				: item.expected_size;
		const contentType = response.headers.get('content-type') ?? item.content_type;
		await hostCall('begin_download_item', {
			request: {
				job_id: jobId,
				item_id: item.id,
				url: item.url,
				resolved_url: source.url,
				expected_size: expectedSize ?? null,
				content_type: contentType ?? null,
				etag: response.headers.get('etag'),
				last_modified: response.headers.get('last-modified'),
				source_host: source.host
			}
		});

		const finalOffset = await streamBody(
			jobId,
			item.id,
			response,
			begin.offset,
			expectedSize,
			controller
		);
		await hostCall('finalize_download_item', {
			request: {
				job_id: jobId,
				item_id: item.id,
				record_id: item.record_id,
				url: item.url,
				resolved_url: source.url,
				expected_size: expectedSize ?? finalOffset,
				content_type: contentType ?? null
			}
		});
		post({ type: 'item-completed', jobId, itemId: item.id });
	} catch (error) {
		const errorClass = classifyDownloadError(error);
		await hostCall('fail_download_item', {
			request: {
				job_id: jobId,
				item_id: item.id,
				error: String(error instanceof Error ? error.message : error),
				error_class: errorClass,
				retryable: ['timeout', 'network', 'blocked', 'corrupt'].includes(errorClass)
			}
		}).catch(() => undefined);
		post({
			type: 'item-failed',
			jobId,
			itemId: item.id,
			error: String(error instanceof Error ? error.message : error),
			errorClass
		});
	} finally {
		controllers.delete(`${jobId}:${item.id}`);
	}
}

async function resolveSource(
	rawUrl: string,
	recordId: string | null,
	_signal: AbortSignal
): Promise<{ url: string; host: string }> {
	if (!rawUrl.startsWith('dvids://asset/')) {
		return { url: rawUrl, host: new URL(rawUrl).host };
	}

	const assetId = rawUrl.slice('dvids://asset/'.length);
	console.log(`[Worker] Resolving DVIDS asset: ${assetId}`);

	// We use the host resolver (hidden webview) to bypass WAF. recordId lets the host look up
	// whether this is a video or audio DVIDS asset (source_asset_class) instead of always
	// requesting the video: namespace. We add a 30-second timeout to prevent permanent hang in
	// the worker thread.
	const resolutionPromise = hostCall(
		'resolve_dvids_metadata',
		buildResolveDvidsMetadataArgs(assetId, recordId)
	);

	const timeoutPromise = new Promise((_, reject) =>
		setTimeout(() => reject(new Error(`DVIDS resolution timed out for ${assetId}`)), 30000)
	);

	const payload = (await Promise.race([resolutionPromise, timeoutPromise])) as unknown;

	const mediaUrl = selectDvidsFileUrl(payload);
	if (!mediaUrl)
		throw new Error(`DVIDS asset ${assetId} did not include a downloadable media file`);
	return { url: mediaUrl, host: new URL(mediaUrl).host };
}

function fetchWithResume(url: string, offset: number, signal: AbortSignal) {
	const headers = new Headers();
	if (offset > 0) headers.set('Range', `bytes=${offset}-`);
	return fetch(url, { headers, signal, cache: 'no-store' });
}

async function streamBody(
	jobId: string,
	itemId: string,
	response: Response,
	initialOffset: number,
	totalBytes: number | null | undefined,
	controller: AbortController
) {
	if (!response.body) throw new Error('Response body is not streamable');
	const reader = response.body.getReader();
	let offset = initialOffset;
	let pendingBytes: Uint8Array[] = [];
	let pendingLength = 0;
	let lastEmitMs = performance.now();
	let lastEmitBytes = offset;

	const flush = async () => {
		if (pendingLength === 0) return;
		const merged = new Uint8Array(pendingLength);
		let cursor = 0;
		for (const chunk of pendingBytes) {
			merged.set(chunk, cursor);
			cursor += chunk.byteLength;
		}
		const result = (await hostCall('append_download_chunk', {
			request: {
				item_id: itemId,
				offset,
				bytes_base64: bytesToBase64(merged)
			}
		})) as AppendResponse;
		offset = result.offset;
		pendingBytes = [];
		pendingLength = 0;
		const nowMs = performance.now();
		if (
			getProgressUpdate({
				nowMs,
				lastEmitMs,
				bytesDownloaded: offset,
				lastEmitBytes
			})
		) {
			lastEmitMs = nowMs;
			lastEmitBytes = offset;
			post({
				type: 'progress',
				jobId,
				itemId,
				bytesDownloaded: offset,
				totalBytes: totalBytes ?? null
			});
		}
	};

	while (true) {
		if (controller.signal.aborted) throw new DOMException('Download cancelled', 'AbortError');
		const next = await reader.read();
		if (next.done) break;
		pendingBytes.push(next.value);
		pendingLength += next.value.byteLength;
		if (pendingLength >= MAX_CHUNK_BYTES) await flush();
	}
	await flush();
	post({
		type: 'progress',
		jobId,
		itemId,
		bytesDownloaded: offset,
		totalBytes: totalBytes ?? null
	});
	return offset;
}

async function hostCall(
	command: HostCommand,
	args: Record<string, unknown>,
	retries = MAX_RETRIES
): Promise<unknown> {
	for (let i = 0; i <= retries; i++) {
		try {
			const id = nextCallId++;
			const promise = new Promise((resolve, reject) => {
				// Internal timeout for the host to acknowledge the call
				const timeout = setTimeout(() => {
					pending.delete(id);
					reject(new Error(`Host call ${command} timed out`));
				}, 60000); // 1 minute timeout per call

				pending.set(id, {
					resolve: (val) => {
						clearTimeout(timeout);
						resolve(val);
					},
					reject: (err) => {
						clearTimeout(timeout);
						reject(err);
					}
				});
			});

			post({ type: 'host-call', id, command, args });
			return await promise;
		} catch (error) {
			if (i === retries) throw error;
			const isTimeout = String(error).includes('timeout');
			if (isTimeout) {
				console.warn(`Host call ${command} failed (attempt ${i + 1}/${retries + 1}), retrying...`);
				await new Promise((r) => setTimeout(r, RETRY_DELAY_MS * (i + 1)));
				continue;
			}
			throw error;
		}
	}
}

function post(message: OutgoingMessage) {
	self.postMessage(message);
}
