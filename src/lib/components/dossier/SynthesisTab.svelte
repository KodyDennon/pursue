<script lang="ts">
	import { Brain } from 'lucide-svelte';
	import { convertFileSrc } from '@tauri-apps/api/core';
	import type { RecordSummary, RecordAsset } from '$lib/types';

	let {
		record,
		images,
		busy,
		onRunDeepSynthesis,
		compact = false
	} = $props<{
		record: RecordSummary;
		images: RecordAsset[];
		busy: string | null;
		onRunDeepSynthesis: () => void;
		compact?: boolean;
	}>();
</script>

<div class="view-padding" class:compact>
	{#if record.intelligence_json}
		{@const intel = JSON.parse(record.intelligence_json)}
		<div class="intel-grid">
			<div class="intel-main">
				<section class="intel-card-section">
					<header class="section-head"><span class="prefix">EXECUTIVE SUMMARY</span></header>
					<p class="para">{intel.object_description || 'No summary available.'}</p>
				</section>

				<div class="data-grid-tactical">
					<div class="t-card">
						<span class="t-label">TARGET DATE</span>
						<span class="t-val">{intel.incident_date || record.incident_date || 'UNDISCLOSED'}</span
						>
					</div>
					<div class="t-card">
						<span class="t-label">GEOSPATIAL TAG</span>
						<span class="t-val">{intel.location || record.incident_location || 'GLOBAL'}</span>
					</div>
					<div class="t-card full">
						<span class="t-label">AGENCY ASSOCIATIONS</span>
						<div class="t-tags">
							{#each intel.agencies || [] as agency (agency)}
								<span class="f-tag">{agency}</span>
							{/each}
						</div>
					</div>
				</div>

				<section class="intel-card-section">
					<header class="section-head"><span class="prefix">QUALITATIVE OBSERVATIONS</span></header>
					<p class="para">{intel.pilot_observations || 'No observational data resolved.'}</p>
				</section>
			</div>

			<aside class="intel-sidebar">
				<div class="fidelity-dial-wrap">
					<span class="t-label">SYNTHESIS FIDELITY</span>
					<div class="dial">
						{Math.round((intel.intelligence_score || 0.6) * 100)}%
					</div>
				</div>
				{#if images.length > 0 && !compact}
					<div class="mini-gallery">
						<span class="t-label">VISUAL EVIDENCE</span>
						<div class="g-grid">
							{#each images.slice(0, 4) as img (img.id)}
								<img src={convertFileSrc(img.local_path)} alt="Evidence" />
							{/each}
						</div>
					</div>
				{/if}
			</aside>
		</div>
	{:else}
		<div class="pending-state">
			<Brain size={48} class="accent-icon" />
			<h3>Deep Intelligence Synthesis Pending</h3>
			<p>Gemma 4 must perform a semantic audit to generate executive intelligence.</p>
			<button class="primary-btn" onclick={onRunDeepSynthesis} disabled={busy === 'synthesis'}>
				RUN NEURAL SYNTHESIS
			</button>
		</div>
	{/if}
</div>

<style>
	.view-padding {
		padding: var(--space-7xl);
	}
	.view-padding.compact {
		padding: var(--space-4xl);
	}
	.section-head {
		margin-bottom: var(--space-4xl);
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
		padding-bottom: var(--space-md);
	}
	.prefix {
		font-size: var(--text-2xs);
		font-weight: 900;
		letter-spacing: 0.15em;
		color: var(--color-text-tertiary);
	}
	.para {
		font-size: var(--text-lg);
		line-height: 1.7;
		color: var(--color-text-primary);
	}
	.data-grid-tactical {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-3xl);
		margin: 32px 0;
	}
	.compact .data-grid-tactical {
		grid-template-columns: 1fr;
	}
	.t-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		padding: var(--space-3xl);
		border-radius: var(--radius-base);
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}
	.t-card.full {
		grid-column: span 2;
	}
	.compact .t-card.full {
		grid-column: span 1;
	}
	.t-label {
		font-size: var(--text-2xs);
		font-weight: 900;
		color: var(--color-text-tertiary);
	}
	.t-val {
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.t-tags {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		margin-top: var(--space-md);
	}
	.fidelity-dial-wrap {
		background: #000;
		border: 1px solid var(--color-border-subtle);
		padding: var(--space-4xl);
		border-radius: var(--radius-md);
		text-align: center;
	}
	.dial {
		font-size: var(--text-6xl);
		font-weight: 800;
		margin-top: var(--space-xl);
		color: var(--color-accent-primary);
	}
	.pending-state {
		height: 400px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: var(--space-3xl);
		color: var(--color-text-tertiary);
	}
	.primary-btn {
		background: var(--color-accent-primary);
		color: #000;
		border: none;
		padding: 12px 24px;
		border-radius: var(--radius-base);
		font-weight: 800;
		cursor: pointer;
	}

	.intel-grid {
		display: flex;
		gap: var(--space-7xl);
	}
	.compact .intel-grid {
		flex-direction: column;
		gap: var(--space-5xl);
	}
	.intel-main {
		flex: 1;
	}
	.intel-sidebar {
		width: 260px;
	}
	.compact .intel-sidebar {
		width: 100%;
	}
	.mini-gallery {
		margin-top: var(--space-7xl);
	}
	.g-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-md);
		margin-top: var(--space-xl);
	}
	.g-grid img {
		width: 100%;
		height: 100px;
		object-fit: cover;
		border-radius: var(--radius-xs);
		border: 1px solid var(--color-border-subtle);
	}
</style>
