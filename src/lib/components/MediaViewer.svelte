<script lang="ts">
	import {
		X,
		ExternalLink,
		Download,
		ZoomIn,
		ZoomOut,
		RotateCw
	} from 'lucide-svelte';
	import { convertFileSrc, invoke } from '@tauri-apps/api/core';
	import { openPath } from '@tauri-apps/plugin-opener';
	import type { RecordSummary } from '$lib/types';

	let { record, isOpen = $bindable(false) } = $props<{
		record: RecordSummary;
		isOpen: boolean;
	}>();

	let assetUrl = $state('');
	let loading = $state(true);
	let zoom = $state(1);
	let rotation = $state(0);
	let isDragging = $state(false);
	let position = $state({ x: 0, y: 0 });
	let startPos = { x: 0, y: 0 };

	async function loadMedia() {
		if (!record.local_path) return;
		loading = true;
		try {
			const fullPath = await invoke<string>('get_record_artifact_path', { id: record.id });
			assetUrl = convertFileSrc(fullPath);
		} catch (e) {
			console.error('Failed to load media:', e);
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		if (isOpen && record.local_path) {
			loadMedia();
		}
	});

	function close() {
		isOpen = false;
		zoom = 1;
		rotation = 0;
		position = { x: 0, y: 0 };
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}

	function handleMouseDown(e: MouseEvent) {
		if (zoom <= 1) return;
		isDragging = true;
		startPos = { x: e.clientX - position.x, y: e.clientY - position.y };
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isDragging) return;
		position = {
			x: e.clientX - startPos.x,
			y: e.clientY - startPos.y
		};
	}

	function handleMouseUp() {
		isDragging = false;
	}

	const fileType = $derived(record.file_type?.toLowerCase() || '');
	const isImage = $derived(
		['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp', 'tif', 'tiff'].includes(fileType)
	);
	const isVideo = $derived(['mp4', 'webm', 'ogg', 'mov'].includes(fileType));
	const isPdf = $derived(fileType === 'pdf');
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isOpen}
	<div
		class="media-overlay"
		role="presentation"
		onclick={(e) => e.target === e.currentTarget && close()}
	>
		<div class="media-container glass-panel">
			<header class="media-header">
				<div class="media-info">
					<span class="file-tag">{fileType.toUpperCase()}</span>
					<h3>{record.title}</h3>
				</div>

				<div class="media-actions">
					{#if isImage}
						<div class="tool-group">
							<button onclick={() => (zoom = Math.max(0.5, zoom - 0.2))} title="Zoom Out"
								><ZoomOut size={18} /></button
							>
							<span class="zoom-pct">{Math.round(zoom * 100)}%</span>
							<button onclick={() => (zoom = Math.min(5, zoom + 0.2))} title="Zoom In"
								><ZoomIn size={18} /></button
							>
							<button onclick={() => (rotation = (rotation + 90) % 360)} title="Rotate"
								><RotateCw size={18} /></button
							>
						</div>
					{/if}

					<button
						class="icon-btn"
						onclick={() =>
							invoke<string>('get_record_artifact_path', { id: record.id }).then((p) =>
								openPath(p)
							)}
						title="Open in System Viewer"
					>
						<ExternalLink size={18} />
					</button>
					<button class="icon-btn close" onclick={close} title="Close">
						<X size={20} />
					</button>
				</div>
			</header>

			<div
				class="media-content"
				role="presentation"
				class:grabbing={isDragging}
				onmousedown={handleMouseDown}
				onmousemove={handleMouseMove}
				onmouseup={handleMouseUp}
				onmouseleave={handleMouseUp}
			>
				{#if loading}
					<div class="media-loader">
						<div class="spinner"></div>
						<span>Decrypting Vault Asset...</span>
					</div>
				{:else if assetUrl}
					{#if isImage}
						<div class="image-wrapper" style="transform: translate({position.x}px, {position.y}px)">
							<img
								src={assetUrl}
								alt={record.title}
								style="transform: scale({zoom}) rotate({rotation}deg)"
								draggable="false"
							/>
						</div>
					{:else if isVideo}
						<video controls autoplay src={assetUrl}>
							<track kind="captions" />
							Your browser does not support the video tag.
						</video>
					{:else if isPdf}
						<iframe src="{assetUrl}#toolbar=0" title="PDF Viewer"></iframe>
					{:else}
						<div class="unsupported-media">
							<div class="file-icon">
								<Download size={48} />
							</div>
							<h3>Unsupported Preview Type</h3>
							<p>Preview is not available for {fileType.toUpperCase()} files within the OS.</p>
							<button
								class="primary-btn"
								onclick={() =>
									invoke<string>('get_record_artifact_path', { id: record.id }).then((p) =>
										openPath(p)
									)}
							>
								Open with External Application
							</button>
						</div>
					{/if}
				{:else}
					<div class="media-error">
						<X size={48} />
						<h3>Failed to Load Asset</h3>
						<p>Ensure the evidence is downloaded to the local vault.</p>
					</div>
				{/if}
			</div>

			<footer class="media-footer">
				<div class="f-item">
					<span class="label">SHA-256:</span>
					<span class="val mono">{record.artifact_sha256?.substring(0, 16) || 'N/A'}...</span>
				</div>
				<div class="f-item">
					<span class="label">VAULT PATH:</span>
					<span class="val mono">{record.local_path}</span>
				</div>
			</footer>
		</div>
	</div>
{/if}

<style>
	.media-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.9);
		backdrop-filter: blur(8px);
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 40px;
	}

	.media-container {
		width: 100%;
		max-width: 1400px;
		height: 100%;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: #000;
		border: 1px solid var(--border-subtle);
		box-shadow: 0 30px 60px -12px rgba(0, 0, 0, 0.5);
	}

	.media-header {
		height: 64px;
		padding: 0 24px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		background: rgba(10, 11, 13, 0.8);
		border-bottom: 1px solid var(--border-subtle);
		z-index: 10;
	}

	.media-info {
		display: flex;
		align-items: center;
		gap: 16px;
	}

	.file-tag {
		font-size: 10px;
		font-weight: 800;
		padding: 2px 6px;
		background: var(--accent-primary);
		color: #000;
		border-radius: 4px;
	}

	.media-info h3 {
		font-size: 16px;
		color: var(--text-primary);
		margin: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 600px;
	}

	.media-actions {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.tool-group {
		display: flex;
		align-items: center;
		gap: 8px;
		background: rgba(255, 255, 255, 0.05);
		padding: 4px 12px;
		border-radius: 20px;
		margin-right: 12px;
	}

	.zoom-pct {
		font-size: 11px;
		font-weight: 700;
		color: var(--text-secondary);
		min-width: 40px;
		text-align: center;
	}

	.tool-group button {
		color: var(--text-secondary);
		transition: color 0.2s;
	}

	.tool-group button:hover {
		color: var(--text-primary);
	}

	.icon-btn {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 8px;
		color: var(--text-secondary);
		transition: all 0.2s;
	}

	.icon-btn:hover {
		background: rgba(255, 255, 255, 0.05);
		color: var(--text-primary);
	}

	.icon-btn.close:hover {
		background: var(--accent-danger);
		color: #fff;
	}

	.media-content {
		flex: 1;
		position: relative;
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: default;
	}

	.media-content.grabbing {
		cursor: grabbing;
	}

	.media-loader {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		color: var(--text-secondary);
	}

	.spinner {
		width: 32px;
		height: 32px;
		border: 3px solid rgba(231, 196, 107, 0.1);
		border-top-color: var(--accent-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.image-wrapper {
		transition: transform 0.1s ease-out;
	}

	img {
		max-width: 90%;
		max-height: 90%;
		object-fit: contain;
		user-select: none;
		pointer-events: none;
		transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
		box-shadow: 0 0 40px rgba(0, 0, 0, 0.5);
	}

	video {
		max-width: 100%;
		max-height: 100%;
		background: #000;
	}

	iframe {
		width: 100%;
		height: 100%;
		border: none;
		background: #fff;
	}

	.unsupported-media,
	.media-error {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		gap: 16px;
		max-width: 400px;
		padding: 40px;
	}

	.file-icon {
		width: 96px;
		height: 96px;
		background: rgba(255, 255, 255, 0.03);
		border-radius: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
	}

	.primary-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 12px 24px;
		border-radius: 8px;
		font-weight: 700;
		cursor: pointer;
	}

	.media-footer {
		height: 32px;
		background: #000;
		border-top: 1px solid var(--border-subtle);
		display: flex;
		align-items: center;
		padding: 0 24px;
		gap: 32px;
	}

	.f-item {
		display: flex;
		gap: 8px;
		align-items: center;
		font-size: 10px;
	}

	.label {
		color: var(--text-tertiary);
	}

	.val {
		color: var(--text-secondary);
	}

	.mono {
		font-family: var(--font-mono);
	}
</style>
