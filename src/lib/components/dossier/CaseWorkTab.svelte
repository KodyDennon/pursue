<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import type { CaseSummary } from '$lib/types';

	let {
		recordId,
		selectedCaseId,
		selectedCase,
		onChanged
	} = $props<{
		recordId: string;
		selectedCaseId: string | null;
		selectedCase: CaseSummary | null;
		onChanged: () => void | Promise<void>;
	}>();

	let noteBody = $state('');
	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	let exportPath = $state<string | null>(null);

	async function addToCase() {
		if (!selectedCaseId) return;
		busy = 'case-add';
		error = null;
		try {
			await invoke('add_record_to_case', {
				request: { case_id: selectedCaseId, record_id: recordId, notes: noteBody.trim() || null }
			});
			if (onChanged) await onChanged();
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	async function addNote() {
		if (!selectedCaseId || !noteBody.trim()) return;
		busy = 'case-note';
		error = null;
		try {
			await invoke('update_case_notes', {
				request: { case_id: selectedCaseId, record_id: recordId, body: noteBody.trim() }
			});
			noteBody = '';
			if (onChanged) await onChanged();
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	async function exportCase(format: 'markdown' | 'html') {
		if (!selectedCaseId) return;
		busy = `export-${format}`;
		error = null;
		try {
			const result = await invoke<{ absolute_path: string }>('export_case', {
				request: { case_id: selectedCaseId, format }
			});
			exportPath = result.absolute_path;
			if (onChanged) await onChanged();
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}
</script>

<div class="view-padding">
	<header class="section-head"><span class="prefix">TACTICAL CASE INTEGRATION</span></header>
	<section class="case-work-section">
		<p class="case-status">
			{selectedCase
				? `Target Case: ${selectedCase.title}`
				: 'No primary case active. Select a case from the Tactical Dashboard.'}
		</p>
		<textarea bind:value={noteBody} rows="5" placeholder="Append forensic observations to case log..."
		></textarea>
		{#if error}
			<p class="error-msg">Error: {error}</p>
		{/if}
		<div class="case-actions">
			<button class="btn-premium" onclick={addToCase} disabled={!selectedCaseId || !!busy}
				>Add to Case</button
			>
			<button
				class="btn-premium"
				onclick={addNote}
				disabled={!selectedCaseId || !noteBody.trim() || !!busy}>Post Note</button
			>
			<button
				class="btn-premium"
				onclick={() => exportCase('markdown')}
				disabled={!selectedCaseId || !!busy}>Export MD</button
			>
			<button
				class="btn-premium"
				onclick={() => exportCase('html')}
				disabled={!selectedCaseId || !!busy}>Export HTML</button
			>
		</div>
		{#if exportPath}
			<p class="path-line">Dossier exported to: {exportPath}</p>
		{/if}
	</section>
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
	.case-work-section {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}
	.case-status {
		font-size: 13px;
		font-weight: 600;
		color: var(--accent-primary);
	}
	.case-work-section textarea {
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		padding: 16px;
		color: #fff;
		font-family: var(--font-display);
		resize: none;
	}
	.case-actions {
		display: flex;
		gap: 12px;
	}
	.path-line {
		font-size: 11px;
		color: var(--text-tertiary);
		font-family: var(--font-mono);
	}
	.error-msg {
		color: var(--accent-error, #ff4d4d);
		font-size: 12px;
	}
</style>
