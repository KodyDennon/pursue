<script lang="ts">
	import type { RecordSummary } from '$lib/types';
	import { formatBytes } from '$lib/utils';
	import {
		FileText,
		CheckCircle2,
		ExternalLink,
		Zap,
		Maximize2,
		Loader2,
		AlertCircle
	} from 'lucide-svelte';

	let {
		records,
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
</script>

<div class="list-view custom-scrollbar">
	{#if records.length === 0}
		<div class="empty-state">No intelligence records match the current filter.</div>
	{:else}
		<table class="intel-table">
			<thead>
				<tr>
					<th class="col-status">Status</th>
					<th class="col-title">Record Title</th>
					<th class="col-agency">Agency</th>
					<th class="col-date">Released</th>
					<th class="col-size">Size</th>
					<th class="col-actions">Source</th>
				</tr>
			</thead>
			<tbody>
				{#each records as record (record.id)}
					<tr class:selected={selectedRecordId === record.id} onclick={() => onSelect(record)}>
						<td class="col-status">
							<div
								class="status-indicator"
								class:ready={record.analysis_status === 'completed'}
								class:pending={record.analysis_status === 'indexing' ||
									record.analysis_status === 'extracting-foundation'}
								class:busy={record.analysis_status === 'synthesizing'}
								class:error={record.analysis_status === 'failed'}
							>
								{#if record.analysis_status === 'completed'}
									<CheckCircle2 size={12} />
								{:else if record.analysis_status === 'synthesizing'}
									<Zap size={12} class="spin" />
								{:else if record.analysis_status === 'indexing' || record.analysis_status === 'extracting-foundation'}
									<Loader2 size={12} class="spin" />
								{:else if record.analysis_status === 'failed'}
									<AlertCircle size={12} />
								{:else}
									<FileText size={12} />
								{/if}
							</div>
						</td>
						<td class="col-title">
							<div class="title-cell">
								<span class="main-title">{record.title}</span>
								<span class="sub-id">{record.id.substring(0, 8)}</span>
							</div>
						</td>
						<td class="col-agency">
							<span class="agency-tag">{record.agency || 'UNKNOWN'}</span>
						</td>
						<td class="col-date">{record.release_date || '--'}</td>
						<td class="col-size">{record.local_path ? formatBytes(record.artifact_size) : '--'}</td>
						<td class="col-actions">
							<div class="row-actions">
								{#if record.document_url}
									<a
										href={record.document_url}
										target="_blank"
										rel="noreferrer"
										class="source-link"
										onclick={(e) => e.stopPropagation()}
										title="Open Remote Source"
									>
										<ExternalLink size={14} />
									</a>
								{/if}
								{#if record.local_path && onView}
									<button
										class="source-link"
										onclick={(e) => {
											e.stopPropagation();
											onView(record);
										}}
										title="View Artifact"
									>
										<Maximize2 size={14} />
									</button>
								{/if}
							</div>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>

<style>
	.list-view {
		height: 100%;
		overflow-y: auto;
	}

	.intel-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 13px;
		text-align: left;
	}

	.intel-table th {
		position: sticky;
		top: 0;
		background: #0a0b0d;
		padding: 12px 16px;
		color: var(--text-tertiary);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-size: 10px;
		border-bottom: 1px solid var(--border-subtle);
		z-index: 1;
	}

	.intel-table tr {
		border-bottom: 1px solid rgba(255, 255, 255, 0.03);
		cursor: pointer;
		transition: background 0.2s;
	}

	.intel-table tr:hover {
		background: rgba(255, 255, 255, 0.03);
	}

	.intel-table tr.selected {
		background: rgba(231, 196, 107, 0.08);
	}

	.intel-table td {
		padding: 12px 16px;
		vertical-align: middle;
	}

	.col-status {
		width: 40px;
	}
	.col-title {
		max-width: 400px;
	}
	.col-agency {
		width: 140px;
	}
	.col-date {
		width: 120px;
		color: var(--text-secondary);
	}
	.col-size {
		width: 100px;
		color: var(--text-tertiary);
		font-family: var(--font-mono);
		font-size: 11px;
	}
	.col-actions {
		width: 80px;
	}

	.status-indicator {
		width: 24px;
		height: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.05);
		color: var(--text-tertiary);
	}

	.status-indicator.ready {
		background: rgba(77, 243, 169, 0.1);
		color: var(--accent-success);
	}

	.status-indicator.busy {
		background: rgba(231, 196, 107, 0.1);
		color: var(--accent-primary);
	}

	.status-indicator.error {
		background: rgba(243, 77, 77, 0.1);
		color: var(--accent-danger);
	}

	.title-cell {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.main-title {
		font-weight: 500;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sub-id {
		font-size: 10px;
		font-family: var(--font-mono);
		color: var(--text-tertiary);
		opacity: 0.7;
	}

	.agency-tag {
		font-size: 10px;
		font-weight: 700;
		background: rgba(255, 255, 255, 0.05);
		padding: 2px 6px;
		border-radius: 4px;
		color: var(--text-secondary);
	}

	.row-actions {
		display: flex;
		gap: 12px;
	}

	.source-link {
		color: var(--text-tertiary);
		transition: color 0.2s;
	}

	.source-link:hover {
		color: var(--accent-primary);
	}

	.empty-state {
		padding: 60px;
		text-align: center;
		color: var(--text-tertiary);
		font-style: italic;
	}
</style>
