<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { ShieldAlert, Crosshair, Eye, EyeOff } from 'lucide-svelte';
	import type { RecordForensics, RecordAsset } from '$lib/types';

	let {
		forensics = [],
		images = []
	} = $props<{
		recordId: string;
		forensics: RecordForensics[];
		images: RecordAsset[];
	}>();

	let selectedImageIdx = $state(0);
	let showXRay = $state(true);
	let hoveredId = $state<string | null>(null);

	const currentImage = $derived(images[selectedImageIdx]);
	const activeForensics = $derived(
		forensics.filter((f: RecordForensics) => f.layer_type !== 'metadata_leak')
	);

	function getBoundingBox(item: RecordForensics) {
		if (!item.bounding_box_json) return null;
		try {
			return JSON.parse(item.bounding_box_json);
		} catch {
			return null;
		}
	}
</script>

<div class="forensic-viewer">
	<div class="viewer-main">
		<div class="stage-container">
			<div class="tactical-hud">
				<div class="hud-top">
					<div class="hud-tag">
						<ShieldAlert size={12} />
						<span>NEURAL AUDIT ACTIVE</span>
					</div>
					<div class="hud-controls">
						<button class="hud-btn" class:active={showXRay} onclick={() => (showXRay = !showXRay)}>
							{#if showXRay}<Eye size={14} />{:else}<EyeOff size={14} />{/if}
							X-RAY MODE
						</button>
					</div>
				</div>

				{#if currentImage}
					<div class="document-stage" class:xray-active={showXRay}>
						<img src={convertFileSrc(currentImage.local_path)} alt="Evidence" class="base-layer" />

						{#if showXRay}
							<svg
								class="forensic-overlay"
								viewBox="0 0 1000 1414"
								preserveAspectRatio="xMidYMid meet"
							>
								{#each activeForensics as item (item.id)}
									{@const box = getBoundingBox(item)}
									{#if box}
										<g
											class="discovery-node"
											role="graphics-symbol"
											aria-label="Forensic Discovery"
											class:hovered={hoveredId === item.id}
											onmouseenter={() => (hoveredId = item.id)}
											onmouseleave={() => (hoveredId = null)}
										>
											<rect
												x={box[0]}
												y={box[1]}
												width={box[2]}
												height={box[3]}
												class="node-rect"
												class:improper={item.layer_type === 'improper_redaction'}
												class:hidden={item.layer_type === 'hidden_text'}
											/>
											<line
												x1={box[0]}
												y1={box[1]}
												x2={box[0] - 20}
												y2={box[1] - 20}
												class="connector"
											/>
											<text x={box[0] - 25} y={box[1] - 25} class="node-label"
												>{item.layer_type.replace('_', ' ').toUpperCase()}</text
											>
										</g>
									{/if}
								{/each}
							</svg>
						{/if}

						<div class="scan-line"></div>
					</div>
				{:else}
					<div class="empty-stage">
						<Crosshair size={48} class="pulse" />
						<p>Awaiting Visual Stream...</p>
					</div>
				{/if}
			</div>
		</div>

		<aside class="discovery-sidebar custom-scrollbar">
			<header class="side-header">
				<span class="prefix">DISCOVERY LOG</span>
				<h2>{activeForensics.length} SENSORY HITS</h2>
			</header>

			<div class="discovery-stack">
				{#each activeForensics as item (item.id)}
					<div
						class="discovery-card"
						role="button"
						tabindex="0"
						class:active={hoveredId === item.id}
						onmouseenter={() => (hoveredId = item.id)}
						onmouseleave={() => (hoveredId = null)}
						onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (hoveredId = item.id)}
					>
						<div class="d-type-tag">{item.layer_type.replace('_', ' ').toUpperCase()}</div>
						<p class="d-snippet">{item.content}</p>
						<div class="d-meta">
							<span class="d-conf">CONF: {Math.round(item.confidence * 100)}%</span>
							<span class="d-src">Neural Reconstruction</span>
						</div>
					</div>
				{/each}
			</div>
		</aside>
	</div>

	{#if images.length > 1}
		<div class="page-nav">
			{#each images as img (img.id)}
				{@const i = images.indexOf(img)}
				<button class:active={selectedImageIdx === i} onclick={() => (selectedImageIdx = i)}>
					PAGE {i + 1}
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.forensic-viewer {
		height: 100%;
		display: flex;
		flex-direction: column;
		background: #000;
	}

	.viewer-main {
		flex: 1;
		display: grid;
		grid-template-columns: 1fr 320px;
		overflow: hidden;
	}

	.stage-container {
		padding: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: radial-gradient(circle at center, #1a1c1e 0%, #000 100%);
		position: relative;
	}

	.tactical-hud {
		width: 100%;
		max-width: 800px;
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.hud-top {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.hud-tag {
		display: flex;
		align-items: center;
		gap: 8px;
		background: rgba(255, 60, 0, 0.1);
		border: 1px solid rgba(255, 60, 0, 0.2);
		padding: 4px 12px;
		border-radius: 4px;
		font-size: 10px;
		font-weight: 900;
		color: #ff3c00;
		letter-spacing: 0.1em;
	}

	.hud-btn {
		background: rgba(255, 255, 255, 0.05);
		border: 1px solid rgba(255, 255, 255, 0.1);
		color: var(--text-tertiary);
		padding: 6px 12px;
		border-radius: 4px;
		font-size: 10px;
		font-weight: 800;
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.hud-btn.active {
		background: var(--accent-primary);
		color: #000;
		border-color: var(--accent-primary);
	}

	.document-stage {
		flex: 1;
		position: relative;
		background: #000;
		border: 1px solid rgba(255, 255, 255, 0.1);
		box-shadow: 0 0 50px rgba(0, 0, 0, 0.5);
		overflow: hidden;
	}

	.base-layer {
		width: 100%;
		height: 100%;
		object-fit: contain;
		transition: filter 0.3s ease;
	}

	.xray-active .base-layer {
		filter: brightness(0.3) contrast(1.2) grayscale(1) invert(0.1);
	}

	.forensic-overlay {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}

	.discovery-node {
		pointer-events: all;
		cursor: crosshair;
	}

	.node-rect {
		fill: none;
		stroke-width: 2;
		stroke-dasharray: 4;
		transition: all 0.2s ease;
	}

	.node-rect.hidden {
		stroke: #ffb300;
		fill: rgba(255, 179, 0, 0.1);
	}
	.node-rect.improper {
		stroke: #ff3c00;
		fill: rgba(255, 60, 0, 0.1);
	}

	.discovery-node.hovered .node-rect {
		fill: rgba(255, 255, 255, 0.1);
		stroke-dasharray: 0;
		stroke-width: 3;
	}

	.connector {
		stroke: rgba(255, 255, 255, 0.2);
		stroke-width: 1;
		opacity: 0;
		transition: opacity 0.2s;
	}
	.node-label {
		fill: #fff;
		font-size: 12px;
		font-weight: 900;
		opacity: 0;
		transition: opacity 0.2s;
		font-family: var(--font-mono);
	}

	.discovery-node.hovered .connector,
	.discovery-node.hovered .node-label {
		opacity: 1;
	}

	.scan-line {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 2px;
		background: linear-gradient(90deg, transparent, var(--accent-primary), transparent);
		box-shadow: 0 0 10px var(--accent-primary);
		animation: scan 4s linear infinite;
		opacity: 0;
		pointer-events: none;
	}

	.xray-active .scan-line {
		opacity: 0.5;
	}

	@keyframes scan {
		0% {
			top: 0;
		}
		100% {
			top: 100%;
		}
	}

	.discovery-sidebar {
		background: #0a0b0d;
		border-left: 1px solid var(--border-subtle);
		padding: 32px;
		display: flex;
		flex-direction: column;
		gap: 32px;
	}

	.side-header h2 {
		font-size: 18px;
		margin: 8px 0;
		color: #fff;
	}
	.prefix {
		font-size: 9px;
		font-weight: 900;
		color: var(--text-tertiary);
		letter-spacing: 0.2em;
	}

	.discovery-stack {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.discovery-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		padding: 16px;
		transition: all 0.2s ease;
		cursor: pointer;
	}

	.discovery-card.active {
		border-color: var(--accent-primary);
		background: rgba(255, 179, 0, 0.05);
		transform: translateX(-4px);
	}

	.d-type-tag {
		font-size: 9px;
		font-weight: 900;
		color: var(--accent-primary);
		margin-bottom: 12px;
		letter-spacing: 0.1em;
	}
	.d-snippet {
		font-family: var(--font-mono);
		font-size: 12px;
		color: #fff;
		line-height: 1.5;
		margin: 0 0 12px 0;
	}
	.d-meta {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		color: var(--text-tertiary);
	}

	.page-nav {
		padding: 16px 40px;
		background: #0a0b0d;
		border-top: 1px solid var(--border-subtle);
		display: flex;
		gap: 12px;
	}

	.page-nav button {
		background: none;
		border: 1px solid var(--border-subtle);
		color: var(--text-tertiary);
		padding: 6px 16px;
		border-radius: 4px;
		font-size: 11px;
		font-weight: 700;
		cursor: pointer;
	}

	.page-nav button.active {
		background: var(--accent-primary);
		color: #000;
		border: none;
	}

	.empty-stage {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 24px;
		color: var(--text-tertiary);
	}
	:global(.pulse) {
		animation: pulse 2s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 0.5;
		}
		50% {
			opacity: 1;
		}
	}
</style>
