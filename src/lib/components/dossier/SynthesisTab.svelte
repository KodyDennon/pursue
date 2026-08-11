<script lang="ts">
	import { Brain, ShieldCheck, AlertCircle, FileText } from 'lucide-svelte';
	import { convertFileSrc } from '@tauri-apps/api/core';
	import type { RecordSummary, RecordAsset, IntelligenceData, ObservationItem } from '$lib/types';

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

	function parseIntelligence(rawJson: string | null): {
		data: IntelligenceData | null;
		observations: ObservationItem[];
		fidelityScore: number;
	} {
		if (!rawJson) return { data: null, observations: [], fidelityScore: 0.6 };
		try {
			const parsed = JSON.parse(rawJson) as IntelligenceData;
			let obs: ObservationItem[] = [];

			if (Array.isArray(parsed.observations)) {
				obs = (parsed.observations as (Record<string, unknown> | string)[]).map((item) => {
					if (typeof item === 'string') {
						return {
							text: item,
							confidence: 0.8,
							evidence_source: 'semantic_index'
						};
					}
					const obj = item as Record<string, unknown>;
					return {
						text: typeof obj.text === 'string' ? obj.text : String(item),
						confidence: typeof obj.confidence === 'number' ? obj.confidence : 0.8,
						evidence_source: typeof obj.evidence_source === 'string' ? obj.evidence_source : 'grounded_evidence',
						caveat: typeof obj.caveat === 'string' ? obj.caveat : undefined
					};
				});
			} else if (parsed.pilot_observations) {
				obs = [
					{
						text: parsed.pilot_observations,
						confidence: parsed.intelligence_score || 0.7,
						evidence_source: 'legacy_observations'
					}
				];
			}

			const avgConf =
				obs.length > 0
					? obs.reduce((sum, o) => sum + (o.confidence || 0.8), 0) / obs.length
					: parsed.intelligence_score || 0.75;

			return {
				data: parsed,
				observations: obs,
				fidelityScore: Math.round(avgConf * 100)
			};
		} catch {
			return { data: null, observations: [], fidelityScore: 0.5 };
		}
	}
</script>

<div class="view-padding" class:compact>
	{#if record.intelligence_json}
		{@const { data: intel, observations, fidelityScore } = parseIntelligence(record.intelligence_json)}
		<div class="intel-grid">
			<div class="intel-main">
				<section class="intel-card-section">
					<header class="section-head">
						<span class="prefix">EXECUTIVE SUMMARY</span>
						{#if intel?.audit_status}
							<span class="audit-badge status-{intel.audit_status}">{intel.audit_status.toUpperCase()}</span>
						{/if}
					</header>
					<p class="para">{intel?.object_description || 'No summary available.'}</p>
				</section>

				<div class="data-grid-tactical">
					<div class="t-card">
						<span class="t-label">TARGET DATE</span>
						<span class="t-val">{intel?.incident_date || record.incident_date || 'UNDISCLOSED'}</span>
					</div>
					<div class="t-card">
						<span class="t-label">GEOSPATIAL TAG</span>
						<span class="t-val">{intel?.location || record.incident_location || 'GLOBAL'}</span>
					</div>
					<div class="t-card full">
						<span class="t-label">AGENCY ASSOCIATIONS</span>
						<div class="t-tags">
							{#if intel?.agencies && intel.agencies.length > 0}
								{#each intel.agencies as agency (agency)}
									<span class="f-tag">{agency}</span>
								{/each}
							{:else if record.agency}
								<span class="f-tag">{record.agency}</span>
							{:else}
								<span class="f-tag muted">UNSPECIFIED</span>
							{/if}
						</div>
					</div>
				</div>

				<section class="intel-card-section">
					<header class="section-head">
						<span class="prefix">QUALITATIVE OBSERVATIONS ({observations.length})</span>
					</header>
					{#if observations.length > 0}
						<div class="obs-list">
							{#each observations as obs, idx (idx)}
								<div class="obs-card">
									<p class="obs-text">{obs.text}</p>
									<div class="obs-meta">
										<span class="source-tag"><FileText size={12} /> {obs.evidence_source}</span>
										<span class="conf-tag"><ShieldCheck size={12} /> Confidence {Math.round((obs.confidence || 0.8) * 100)}%</span>
									</div>
									{#if obs.caveat}
										<p class="obs-caveat"><AlertCircle size={12} /> {obs.caveat}</p>
									{/if}
								</div>
							{/each}
						</div>
					{:else}
						<p class="para muted">No observational data resolved.</p>
					{/if}
				</section>

				{#if intel?.caveats && intel.caveats.length > 0}
					<section class="intel-card-section caveats-section">
						<header class="section-head"><span class="prefix">INTELLIGENCE CAVEATS</span></header>
						<ul class="caveat-list">
							{#each intel.caveats as caveat (caveat)}
								<li>{caveat}</li>
							{/each}
						</ul>
					</section>
				{/if}
			</div>

			<aside class="intel-sidebar">
				<div class="fidelity-dial-wrap">
					<span class="t-label">SYNTHESIS FIDELITY</span>
					<div class="dial">{fidelityScore}%</div>
					{#if intel?.runtime_device}
						<span class="device-label">Device: {intel.runtime_device}</span>
					{/if}
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
			<h3>Evidence Synthesis Pending</h3>
			<p>Gemma 4 must generate evidence-grounded synthesis for this record.</p>
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

	.audit-badge {
		font-size: var(--text-3xs, 9px);
		font-weight: 800;
		padding: 2px 8px;
		border-radius: var(--radius-full, 9999px);
		margin-left: 12px;
		letter-spacing: 0.08em;
	}
	.audit-badge.status-completed {
		background: rgba(46, 204, 113, 0.15);
		color: #2ecc71;
		border: 1px solid rgba(46, 204, 113, 0.3);
	}
	.audit-badge.status-partial {
		background: rgba(241, 196, 15, 0.15);
		color: #f1c40f;
		border: 1px solid rgba(241, 196, 15, 0.3);
	}

	.obs-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}
	.obs-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-base);
		padding: var(--space-4xl);
	}
	.obs-text {
		font-size: var(--text-md);
		line-height: 1.6;
		color: var(--color-text-primary);
		margin-bottom: var(--space-md);
	}
	.obs-meta {
		display: flex;
		gap: var(--space-xl);
		align-items: center;
		font-size: var(--text-2xs);
	}
	.source-tag {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--color-text-tertiary);
		font-family: var(--font-mono, monospace);
		text-transform: uppercase;
	}
	.conf-tag {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--color-accent-primary);
		font-weight: 700;
	}
	.obs-caveat {
		margin-top: var(--space-md);
		font-size: var(--text-xs);
		color: var(--color-accent-danger, #ff4d4d);
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.caveat-list {
		list-style-type: square;
		padding-left: var(--space-4xl);
		color: var(--color-text-tertiary);
		font-size: var(--text-sm);
		line-height: 1.6;
	}
	.device-label {
		display: block;
		margin-top: 8px;
		font-size: var(--text-3xs, 10px);
		color: var(--color-text-tertiary);
		font-family: var(--font-mono, monospace);
	}
</style>
