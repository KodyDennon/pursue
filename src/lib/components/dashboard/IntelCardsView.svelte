<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import type { RecordSummary } from '$lib/types';
	import { formatBytes } from '$lib/utils';
	import {
		FileText,
		MapPin,
		Calendar,
		Database,
		CheckCircle2,
		ShieldCheck,
		Clock,
		Zap,
		Maximize2,
		Loader2,
		AlertCircle
	} from 'lucide-svelte';
	let {
		records,
		libraryPath = null,
		selectedRecordId = null,
		onSelect,
		onView
	} = $props<{
		records: RecordSummary[];
		libraryPath?: string | null;
		selectedRecordId?: string | null;
		onSelect: (record: RecordSummary) => void;
		onView?: (record: RecordSummary) => void;
	}>();

	function resolvePath(rel: string | null) {
		if (!rel || !libraryPath) return '';
		const cleanLib =
			libraryPath.endsWith('/') || libraryPath.endsWith('\\') ? libraryPath : libraryPath + '/';
		return convertFileSrc(cleanLib + rel);
	}
</script>

<div class="cards-view custom-scrollbar">
	{#if records.length === 0}
		<div class="empty-intel">No intelligence records match the current filter.</div>
	{:else}
		<div class="cards-grid">
			{#each records as record (record.id)}
				<div
					role="button"
					tabindex="0"
					class="intel-card"
					class:active={selectedRecordId === record.id}
					onclick={() => onSelect(record)}
					onkeydown={(e) => e.key === 'Enter' && onSelect(record)}
				>
					{#if record.thumbnail_path}
						<div class="card-thumb">
							<img src={resolvePath(record.thumbnail_path)} alt="Evidence" />
							<div class="thumb-overlay">
								{#if record.local_path && onView}
									<button
										class="thumb-view-btn"
										onclick={(e) => {
											e.stopPropagation();
											onView(record);
										}}
										onkeydown={(e) => e.stopPropagation()}
										title="Quick Preview"
									>
										<Maximize2 size={20} />
									</button>
								{/if}
							</div>
						</div>
					{:else}
						<div class="card-thumb-empty">
							<FileText size={40} strokeWidth={1} />
						</div>
					{/if}

					<div class="card-content">
						<header>
							<span class="agency">{record.agency || 'AARO_OFFICIAL'}</span>
							<div
								class="status"
								class:ready={record.analysis_status === 'completed'}
								class:indexed={record.analysis_status === 'indexed'}
								class:busy={record.analysis_status === 'synthesizing'}
								class:pending={record.analysis_status === 'indexing' || record.analysis_status === 'extracting-foundation'}
								class:error={record.analysis_status === 'failed'}
							>
								{#if record.analysis_status === 'completed'}
									<CheckCircle2 size={10} /> <span>READY</span>
								{:else if record.analysis_status === 'indexed'}
									<ShieldCheck size={10} /> <span>FOUNDATION</span>
								{:else if record.analysis_status === 'synthesizing'}
									<Zap size={10} class="spin" /> <span>NEURAL</span>
								{:else if record.analysis_status === 'indexing' || record.analysis_status === 'extracting-foundation'}
									<Loader2 size={10} class="spin" /> <span>FOUNDATION</span>
								{:else if record.analysis_status === 'failed'}
									<AlertCircle size={10} /> <span>FAILED</span>
								{:else}
									<Clock size={10} />
									<span>PENDING</span>
								{/if}
							</div>
						</header>

						<h3>{record.title}</h3>
						<p class="summary">
							{record.summary || 'Archival record awaiting deep neural extraction...'}
						</p>

						<div class="meta-grid">
							<div class="meta-item">
								<MapPin size={12} />
								<span>{record.incident_location || 'Global'}</span>
							</div>
							<div class="meta-item">
								<Calendar size={12} />
								<span>{record.release_date || 'Undated'}</span>
							</div>
						</div>

						<footer>
							<div class="source">
								<Database size={10} />
								<span>{record.source_type}</span>
							</div>
							<span class="size"
								>{record.local_path ? formatBytes(record.artifact_size) : 'Cloud'}</span
							>
						</footer>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.cards-view {
		height: 100%;
		overflow-y: auto;
		padding: 32px;
	}

	.cards-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
		gap: 24px;
	}

	.intel-card {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		overflow: hidden;
		display: flex;
		flex-direction: column;
		text-align: left;
		transition: var(--transition-normal);
		cursor: pointer;
	}

	.intel-card:hover {
		transform: translateY(-4px);
		border-color: var(--accent-primary);
		background: var(--bg-surface-elevated);
		box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.5);
	}

	.intel-card.active {
		border-color: var(--accent-primary);
		box-shadow: 0 0 0 2px rgba(231, 196, 107, 0.2);
	}

	.card-thumb,
	.card-thumb-empty {
		width: 100%;
		aspect-ratio: 16/9;
		background: #000;
		position: relative;
		overflow: hidden;
	}

	.card-thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		opacity: 0.7;
		transition: transform 0.6s ease;
	}

	.intel-card:hover .card-thumb img {
		transform: scale(1.05);
		opacity: 1;
	}

	.card-thumb-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
		background: radial-gradient(circle at center, #111 0%, #000 100%);
	}

	.thumb-overlay {
		position: absolute;
		inset: 0;
		background: linear-gradient(to bottom, transparent 40%, rgba(0, 0, 0, 0.8));
	}

	.card-content {
		padding: 24px;
		display: flex;
		flex-direction: column;
		flex: 1;
	}

	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 16px;
	}

	.agency {
		font-size: 10px;
		font-weight: 800;
		letter-spacing: 0.15em;
		color: var(--accent-primary);
		text-transform: uppercase;
	}

	.status {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 9px;
		font-weight: 700;
		color: var(--text-tertiary);
		background: rgba(255, 255, 255, 0.05);
		padding: 2px 8px;
		border-radius: 4px;
	}

	.status.ready {
		color: var(--accent-success);
		background: rgba(77, 243, 169, 0.1);
	}

	.status.indexed {
		color: #3296ff;
		background: rgba(50, 150, 255, 0.1);
	}

	.status.busy {
		color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.1);
	}

	.status.pending {
		color: #3296ff;
		background: rgba(50, 150, 255, 0.1);
	}

	.status.error {
		color: var(--accent-danger);
		background: rgba(243, 77, 77, 0.1);
	}

	h3 {
		font-size: 16px;
		font-weight: 600;
		margin: 0 0 12px 0;
		line-height: 1.4;
		color: var(--text-primary);
		display: -webkit-box;
		line-clamp: 2;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.summary {
		font-size: 13px;
		line-height: 1.5;
		color: var(--text-secondary);
		margin: 0 0 20px 0;
		display: -webkit-box;
		line-clamp: 3;
		-webkit-line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
		flex: 1;
	}

	.meta-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 12px;
		margin-bottom: 24px;
	}

	.meta-item {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 11px;
		color: var(--text-tertiary);
	}

	footer {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding-top: 16px;
		border-top: 1px solid var(--border-subtle);
	}

	.source {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
	}

	.size {
		font-size: 10px;
		color: var(--text-tertiary);
	}

	.empty-intel {
		padding: 100px;
		text-align: center;
		color: var(--text-tertiary);
		font-style: italic;
	}
</style>
