import {
	buildResolveDvidsMetadataArgs,
	classifyDownloadError,
	getDownloadDriver,
	getProgressUpdate,
	selectDvidsFileUrl,
	type DownloadErrorClass
} from './downloadWorkerCore';
import type { BulkDownloadItem } from '$lib/types';

const DEFAULT_CONCURRENCY = 3;
const MAX_CHUNK_BYTES = 512 * 1024;

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
	try {
		const concurrency = Math.min(4, Math.max(1, message.concurrency ?? DEFAULT_CONCURRENCY));
		const queue = [...message.items];
		const workers = Array.from({ length: concurrency }, async () => {
			while (queue.length > 0) {
				const item = queue.shift();
				if (!item) return;
				await downloadItem(message.jobId, item);
			}
		});
		await Promise.all(workers);
	} finally {
		isRunning = false;
		post({ type: 'idle', jobId: message.jobId });
	}
}

async function downloadItem(jobId: string, item: BulkDownloadItem) {
	const controller = new AbortController();
	controllers.set(`${jobId}:${item.id}`, controller);
	try {
		if (!item.url) throw new Error('No source URL available');
		const source = await resolveSource(item.url, controller.signal);
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
	_signal: AbortSignal
): Promise<{ url: string; host: string }> {
	if (!rawUrl.startsWith('dvids://asset/')) {
		return { url: rawUrl, host: new URL(rawUrl).host };
	}

	const assetId = rawUrl.slice('dvids://asset/'.length);
	// We use the host resolver (hidden webview) to bypass WAF
	const payload = (await hostCall(
		'resolve_dvids_metadata',
		buildResolveDvidsMetadataArgs(assetId)
	)) as unknown;

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
				bytes: merged
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

function hostCall(command: HostCommand, args: Record<string, unknown>): Promise<unknown> {
	const id = nextCallId++;
	post({ type: 'host-call', id, command, args });
	return new Promise((resolve, reject) => {
		pending.set(id, { resolve, reject });
	});
}

function post(message: OutgoingMessage) {
	self.postMessage(message);
}
