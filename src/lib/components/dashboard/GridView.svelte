<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import type { RecordSummary } from '$lib/types';
	import { FileText, MapPin, Calendar, CheckCircle2, Clock, Zap, Maximize2 } from 'lucide-svelte';

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
		// Ensure we have a clean join
		const cleanLib =
			libraryPath.endsWith('/') || libraryPath.endsWith('\\') ? libraryPath : libraryPath + '/';
		return convertFileSrc(cleanLib + rel);
	}

	function formatBytes(value: number | null | undefined) {
		if (!value) return '0 B';
		const units = ['B', 'KB', 'MB', 'GB', 'TB'];
		let next = value;
		let unit = 0;
		while (next >= 1024 && unit < units.length - 1) {
			next /= 1024;
			unit += 1;
		}
		return `${next.toFixed(next >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
	}
</script>

<div class="cards-view custom-scrollbar">
	<div class="cards-grid">
		{#each records as record (record.id)}
			<div
				role="button"
				tabindex="0"
				class="evidence-card"
				class:selected={selectedRecordId === record.id}
				onclick={() => onSelect(record)}
				onkeydown={(e) => e.key === 'Enter' && onSelect(record)}
			>
				<div class="card-glow"></div>
				<div class="corner-bracket tl"></div>
				<div class="corner-bracket tr"></div>
				<div class="corner-bracket bl"></div>
				<div class="corner-bracket br"></div>
				<div class="scanning-line"></div>

				{#if record.thumbnail_path}
					<div class="card-preview">
						<img src={resolvePath(record.thumbnail_path)} alt="Preview" />
						<div class="preview-overlay">
							{#if record.local_path && onView}
								<button
									class="view-overlay-btn"
									onclick={(e) => {
										e.stopPropagation();
										onView(record);
									}}
									onkeydown={(e) => e.stopPropagation()}
									title="Quick Preview"
								>
									<Maximize2 size={24} />
								</button>
							{/if}
						</div>
					</div>
				{/if}

				<header class="card-header">
					{#if !record.thumbnail_path}
						<div class="type-icon">
							<FileText size={16} />
						</div>
					{/if}
					<span class="agency-tag">{record.agency || 'AARO_OFFICIAL'}</span>
					<div
						class="status-indicator"
						class:completed={record.analysis_status === 'completed'}
						class:indexed={record.analysis_status === 'indexed' ||
							record.analysis_status === 'indexing'}
					>
						{#if record.analysis_status === 'completed'}
							<CheckCircle2 size={12} />
						{:else if record.analysis_status === 'indexed' || record.analysis_status === 'indexing'}
							<Zap size={12} />
						{:else}
							<Clock size={12} />
						{/if}
						<span>{record.analysis_status?.toUpperCase() || 'pending'}</span>
					</div>
				</header>

				<div class="card-body">
					<h3>{record.title}</h3>
					<div class="meta-row">
						<div class="meta-item">
							<MapPin size={12} />
							<span>{record.incident_location || 'Unknown location'}</span>
						</div>
						<div class="meta-item">
							<Calendar size={12} />
							<span>{record.release_date || 'Undated'}</span>
						</div>
					</div>
				</div>

				<footer class="card-footer">
					<span class="file-info"
						>{record.file_type || 'PDF'} • {record.local_path
							? formatBytes(record.artifact_size)
							: 'Cloud Source'}</span
					>
					<div
						class="intel-tag"
						class:active={record.analysis_status === 'completed'}
						class:indexed={record.analysis_status === 'indexed'}
					>
						{#if record.analysis_status === 'completed'}
							INTELLIGENCE READY
						{:else if record.analysis_status === 'indexed'}
							FOUNDATION INDEXED
						{:else}
							AWAITING ANALYSIS
						{/if}
					</div>
				</footer>
			</div>
		{/each}
	</div>
</div>

<style>
	.cards-view {
		height: 100%;
		overflow-y: auto;
		padding: 24px;
		box-sizing: border-box;
	}

	.cards-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: 20px;
	}

	.evidence-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
		text-align: left;
		position: relative;
		overflow: hidden;
		transition: var(--transition-normal);
	}

	.evidence-card:hover {
		background: rgba(255, 255, 255, 0.04);
		border-color: var(--accent-primary);
		transform: translateY(-2px);
	}

	.evidence-card.selected {
		background: rgba(231, 196, 107, 0.05);
		border-color: var(--accent-primary);
		box-shadow: 0 0 20px rgba(231, 196, 107, 0.1);
	}

	.card-glow {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: radial-gradient(circle at 50% 0%, rgba(231, 196, 107, 0.05), transparent 70%);
		opacity: 0;
		transition: opacity 0.3s ease;
		pointer-events: none;
	}

	.evidence-card:hover .card-glow {
		opacity: 1;
	}

	.corner-bracket {
		position: absolute;
		width: 10px;
		height: 10px;
		border: 1px solid rgba(231, 196, 107, 0.3);
		pointer-events: none;
		opacity: 0;
		transition: opacity 0.3s;
	}
	.corner-bracket.tl {
		top: 12px;
		left: 12px;
		border-right: none;
		border-bottom: none;
	}
	.corner-bracket.tr {
		top: 12px;
		right: 12px;
		border-left: none;
		border-bottom: none;
	}
	.corner-bracket.bl {
		bottom: 12px;
		left: 12px;
		border-right: none;
		border-top: none;
	}
	.corner-bracket.br {
		bottom: 12px;
		right: 12px;
		border-left: none;
		border-top: none;
	}
	.evidence-card:hover .corner-bracket {
		opacity: 1;
	}

	.scanning-line {
		position: absolute;
		top: -10%;
		left: 0;
		width: 100%;
		height: 2px;
		background: linear-gradient(90deg, transparent, var(--accent-primary), transparent);
		box-shadow: 0 0 15px var(--accent-primary);
		opacity: 0;
		pointer-events: none;
		z-index: 5;
	}
	.evidence-card:hover .scanning-line {
		animation: scan 2.5s linear infinite;
		opacity: 0.4;
	}

	@keyframes scan {
		0% {
			top: -10%;
		}
		100% {
			top: 110%;
		}
	}

	.card-preview {
		width: 100%;
		height: 120px;
		background: #000;
		overflow: hidden;
		position: relative;
		border-bottom: 1px solid var(--border-subtle);
	}

	.card-preview img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		opacity: 0.8;
		transition: transform 0.4s ease;
	}

	.evidence-card:hover .card-preview img {
		transform: scale(1.05);
		opacity: 1;
	}

	.preview-overlay {
		position: absolute;
		inset: 0;
		background: linear-gradient(to bottom, transparent 60%, rgba(0, 0, 0, 0.6));
	}

	.card-header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 16px 20px 0;
	}

	.type-icon {
		width: 24px;
		height: 24px;
		border-radius: 6px;
		background: rgba(255, 255, 255, 0.05);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-secondary);
	}

	.agency-tag {
		font-size: 10px;
		font-weight: 700;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.status-indicator {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 9px;
		text-transform: uppercase;
		font-weight: 700;
		padding: 2px 6px;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.05);
		color: var(--text-tertiary);
	}

	.status-indicator.completed {
		background: rgba(77, 243, 169, 0.1);
		color: var(--accent-success);
	}

	.status-indicator.indexed {
		background: rgba(50, 150, 255, 0.1);
		color: #3296ff;
	}

	.card-body {
		padding: 12px 20px 20px;
	}

	.card-body h3 {
		font-size: 15px;
		font-weight: 600;
		margin: 0;
		color: var(--text-primary);
		line-height: 1.4;
		display: -webkit-box;
		line-clamp: 2;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.meta-row {
		margin-top: 12px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.meta-item {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 11px;
		color: var(--text-secondary);
	}

	.card-footer {
		margin-top: auto;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 20px;
		background: rgba(255, 255, 255, 0.01);
		border-top: 1px solid rgba(255, 255, 255, 0.03);
	}

	.file-info {
		font-size: 10px;
		color: var(--text-tertiary);
	}

	.intel-tag {
		font-size: 8px;
		font-weight: 800;
		letter-spacing: 0.1em;
		padding: 2px 6px;
		border-radius: 2px;
		background: rgba(255, 255, 255, 0.05);
		color: rgba(255, 255, 255, 0.1);
		transition: var(--transition-normal);
	}

	.intel-tag.active {
		background: var(--accent-primary);
		color: #000;
		box-shadow: 0 0 10px rgba(231, 196, 107, 0.3);
	}

	.intel-tag.indexed {
		background: rgba(50, 150, 255, 0.15);
		color: #3296ff;
	}
</style>
