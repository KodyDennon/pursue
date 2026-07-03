<script lang="ts">
	import {
		Brain,
		Layers,
		Fingerprint,
		Database,
		Clock,
		AlertCircle,
		Activity
	} from 'lucide-svelte';
	import type { RecordSummary, AnalysisReport } from '$lib/types';

	let {
		record,
		analysis,
		activeDomain = $bindable(),
		isSynthesisOutdated,
		analysisStatus,
		analysisProgress,
		runDeepSynthesis
	} = $props<{
		record: RecordSummary;
		analysis: AnalysisReport | null;
		activeDomain: 'intelligence' | 'foundation';
		isSynthesisOutdated: boolean;
		analysisStatus: string | null;
		analysisProgress: number;
		runDeepSynthesis: () => void;
	}>();
</script>

<aside class="dossier-sidebar">
	<div class="sidebar-section">
		<h3 class="section-label">Operational Domain</h3>
		<div class="domain-selector">
			<button
				class:active={activeDomain === 'intelligence'}
				onclick={() => (activeDomain = 'intelligence')}
			>
				<Brain size={14} /> INTELLIGENCE
			</button>
			<button
				class:active={activeDomain === 'foundation'}
				onclick={() => (activeDomain = 'foundation')}
			>
				<Layers size={14} /> FOUNDATION
			</button>
		</div>
	</div>

	<div class="sidebar-section">
		<h3 class="section-label">Target Intelligence</h3>
		<div class="meta-grid">
			<div class="meta-item">
				<span class="label"><Fingerprint size={12} /> ID</span>
				<span class="value">{record.id.substring(0, 12)}...</span>
			</div>
			<div class="meta-item">
				<span class="label"><Database size={12} /> AGENCY</span>
				<span class="value">{record.agency || 'UNKNOWN'}</span>
			</div>
			<div class="meta-item">
				<span class="label"><Clock size={12} /> RELEASED</span>
				<span class="value">{record.release_date || 'UNDATED'}</span>
			</div>
			<div class="meta-item">
				<span class="label">SOURCE TYPE</span>
				<span class="source-tag">{record.source_type.toUpperCase()}</span>
			</div>
			<div class="meta-item">
				<span class="label">SYSTEM ENGINE</span>
				<span class="engine-tag">{analysis?.engine?.toUpperCase() || 'CORE_SYSTEM'}</span>
			</div>
		</div>
	</div>

	<div class="sidebar-section">
		<h3 class="section-label">System Status</h3>
		<div class="status-item" class:completed={record.analysis_status === 'completed'}>
			<span class="label">ANALYSIS</span>
			<span class="value">{record.analysis_status?.toUpperCase() || 'PENDING'}</span>
		</div>
		{#if record.local_path}
			<div class="status-item success">
				<span class="label">ARTIFACT</span>
				<span class="value">VERIFIED LOCAL</span>
			</div>
		{:else}
			<div class="status-item warning">
				<span class="label">ARTIFACT</span>
				<span class="value">REMOTE TARGET</span>
			</div>
		{/if}
	</div>

	{#if analysisStatus}
		<div class="sidebar-section">
			<div class="analysis-hud">
				<div class="hud-label">
					<Activity size={10} class="pulse" />
					<span>{analysisStatus.toUpperCase()}</span>
					<span class="pct">{analysisProgress}%</span>
				</div>
				<div class="hud-bar">
					<div class="hud-fill" style="width: {analysisProgress}%"></div>
				</div>
			</div>
		</div>
	{/if}

	{#if isSynthesisOutdated}
		<div class="sidebar-alert warning">
			<AlertCircle size={14} />
			<div class="alert-content">
				<p>FOUNDATION UPDATED</p>
				<span>New OCR data available.</span>
				<button onclick={runDeepSynthesis}>SYNTHESIZE</button>
			</div>
		</div>
	{/if}
</aside>

<style>
	.dossier-sidebar {
		width: 300px;
		background: rgba(8, 9, 12, 0.4);
		border-left: 1px solid var(--border-subtle);
		display: flex;
		flex-direction: column;
		padding: var(--space-5xl);
		gap: var(--space-7xl);
		overflow-y: auto;
		height: 100%;
	}

	.sidebar-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.section-label {
		font-size: var(--text-xs);
		font-weight: 800;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		margin-bottom: var(--space-xs);
	}

	.domain-selector {
		display: flex;
		flex-direction: column;
		background: rgba(0, 0, 0, 0.2);
		padding: var(--space-xs);
		border-radius: var(--radius-base);
		border: 1px solid var(--border-subtle);
		gap: var(--space-xs);
	}

	.domain-selector button {
		padding: 10px 14px;
		font-size: var(--text-sm);
		font-weight: 700;
		border-radius: var(--radius-sm);
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		border: none;
		background: none;
		cursor: pointer;
		transition: all 0.2s;
		text-align: left;
	}

	.domain-selector button.active {
		background: rgba(231, 196, 107, 0.1);
		color: var(--accent-primary);
		box-shadow: inset 0 0 12px rgba(231, 196, 107, 0.05);
	}

	.meta-grid {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.meta-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.meta-item .label {
		font-size: var(--text-2xs);
		font-weight: 700;
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.meta-item .value {
		font-size: var(--text-base);
		font-family: var(--font-mono);
		color: var(--text-secondary);
	}

	.source-tag,
	.engine-tag {
		font-size: var(--text-2xs);
		font-weight: 900;
		padding: 2px 6px;
		border-radius: 3px;
		background: rgba(255, 255, 255, 0.05);
		color: var(--accent-primary);
		width: fit-content;
	}

	.status-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 8px 0;
		border-bottom: 1px solid var(--border-subtle);
	}

	.status-item .label {
		font-size: var(--text-xs);
		font-weight: 700;
		color: var(--text-tertiary);
	}

	.status-item .value {
		font-size: var(--text-xs);
		font-weight: 900;
		color: var(--text-secondary);
	}

	.status-item.completed .value,
	.status-item.success .value {
		color: var(--accent-success);
	}

	.status-item.warning .value {
		color: var(--accent-primary);
	}

	.analysis-hud {
		background: rgba(231, 196, 107, 0.03);
		border: 1px solid rgba(231, 196, 107, 0.1);
		border-radius: var(--radius-base);
		padding: var(--space-xl);
	}

	.hud-label {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		font-size: var(--text-2xs);
		font-weight: 900;
		color: var(--accent-primary);
		letter-spacing: 0.1em;
		margin-bottom: var(--space-md);
	}

	.hud-bar {
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: var(--radius-2xs);
		overflow: hidden;
	}

	.hud-fill {
		height: 100%;
		background: var(--accent-primary);
		box-shadow: 0 0 10px var(--accent-primary-glow);
		transition: width 0.3s ease;
	}

	.sidebar-alert {
		margin-top: auto;
		padding: var(--space-3xl);
		border-radius: var(--radius-base);
		display: flex;
		gap: var(--space-xl);
		background: rgba(231, 196, 107, 0.05);
		border: 1px solid rgba(231, 196, 107, 0.2);
		color: var(--accent-primary);
	}

	.alert-content p {
		font-size: var(--text-sm);
		font-weight: 900;
		margin-bottom: var(--space-2xs);
	}

	.alert-content span {
		font-size: var(--text-xs);
		opacity: 0.8;
		display: block;
		margin-bottom: var(--space-lg);
	}

	.alert-content button {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 6px 12px;
		border-radius: var(--radius-xs);
		font-size: var(--text-xs);
		font-weight: 900;
		cursor: pointer;
		width: 100%;
	}

	:global(.pulse) {
		animation: pulse-light 1.5s infinite ease-in-out;
	}

	@keyframes pulse-light {
		0%,
		100% {
			opacity: 0.6;
		}
		50% {
			opacity: 1;
			transform: scale(1.05);
		}
	}
</style>
