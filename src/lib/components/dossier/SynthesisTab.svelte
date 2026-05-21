<script lang="ts">
	import { Brain } from 'lucide-svelte';
	import { convertFileSrc } from '@tauri-apps/api/core';
	import type { RecordSummary, AnalysisReport, RecordAsset } from '$lib/types';

	let {
		record,
		analysis,
		images,
		busy,
		onRunDeepSynthesis
	} = $props<{
		record: RecordSummary;
		analysis: AnalysisReport | null;
		images: RecordAsset[];
		busy: string | null;
		onRunDeepSynthesis: () => void;
	}>();
</script>

<div class="view-padding">
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
						<span class="t-val">{intel.incident_date || record.incident_date || 'UNDISCLOSED'}</span>
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
				{#if images.length > 0}
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
		padding: 32px;
	}
	.section-head {
		margin-bottom: 20px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
		padding-bottom: 8px;
	}
	.prefix {
		font-size: 9px;
		font-weight: 900;
		letter-spacing: 0.15em;
		color: var(--text-tertiary);
	}
	.para {
		font-size: 14px;
		line-height: 1.7;
		color: var(--text-primary);
	}
	.data-grid-tactical {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		margin: 32px 0;
	}
	.t-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		padding: 16px;
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.t-card.full { grid-column: span 2; }
	.t-label {
		font-size: 9px;
		font-weight: 900;
		color: var(--text-tertiary);
	}
	.t-val {
		font-size: 14px;
		font-weight: 600;
	}
	.t-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 8px;
	}
	.fidelity-dial-wrap {
		background: #000;
		border: 1px solid var(--border-subtle);
		padding: 20px;
		border-radius: 12px;
		text-align: center;
	}
	.dial {
		font-size: 32px;
		font-weight: 800;
		margin-top: 12px;
		color: var(--accent-primary);
	}
	.pending-state {
		height: 400px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: 16px;
		color: var(--text-tertiary);
	}
	.primary-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 12px 24px;
		border-radius: 8px;
		font-weight: 800;
		cursor: pointer;
	}
</style>
