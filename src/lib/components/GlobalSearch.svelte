<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Search } from 'lucide-svelte';
	import { appStore } from '$lib/stores/appStore.svelte';
	import type { SearchResults } from '$lib/types';
	import { logger } from '$lib/logger';
	import Modal from './Modal.svelte';

	let query = $state('');
	let results = $state<SearchResults | null>(null);
	let loading = $state(false);
	let searchInput = $state<HTMLInputElement>();
	let debounceTimer: ReturnType<typeof setTimeout> | undefined;

	onMount(() => {
		logger.debug('[GlobalSearch] Mounted.');
		// Only the "open" shortcut lives here now — Escape-to-close and click-outside-to-close
		// are handled by <Modal> below, the same as every other modal in the app.
		const handleKeydown = (e: KeyboardEvent) => {
			if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
				e.preventDefault();
				appStore.globalSearchOpen = true;
			}
		};
		window.addEventListener('keydown', handleKeydown);
		return () => window.removeEventListener('keydown', handleKeydown);
	});

	$effect(() => {
		if (appStore.globalSearchOpen && searchInput) {
			setTimeout(() => searchInput?.focus(), 50);
		}
	});

	async function performSearch() {
		const searchedQuery = query.trim();
		if (!searchedQuery) {
			results = null;
			return;
		}
		loading = true;
		try {
			const response = await invoke<SearchResults>('search', {
				request: { query: searchedQuery, filters: {} }
			});
			// A later keystroke may have already started a newer search while this one was
			// in flight — don't let a slow, stale response overwrite fresher results.
			if (query.trim() === searchedQuery) {
				results = response;
			}
		} catch (e) {
			console.error(e);
		} finally {
			if (query.trim() === searchedQuery) {
				loading = false;
			}
		}
	}
</script>

<Modal
	bind:isOpen={appStore.globalSearchOpen}
	zIndex={1000}
	background="rgba(0, 0, 0, 0.6)"
	blur="4px"
	padding="0"
>
	<div class="search-modal-wrap">
		<div class="search-modal glass-panel">
			<div class="search-bar">
				<Search size={20} class="search-icon" />
				<input
					bind:this={searchInput}
					bind:value={query}
					placeholder="Query intelligence index..."
					oninput={() => {
						clearTimeout(debounceTimer);
						debounceTimer = setTimeout(() => {
							if (query.trim().length > 2) performSearch();
						}, 300);
					}}
				/>
				{#if loading}
					<div class="spinner"></div>
				{/if}
			</div>

			<div class="search-results">
				{#if results && results.results.length > 0}
					<div class="results-meta">
						Found {results.total} matches
					</div>
					{#each results.results as result (result.id)}
						<button
							class="result-row"
							onclick={() => {
								appStore.selectedRecordId = result.id;
								appStore.activeView = 'dashboard';
								appStore.globalSearchOpen = false;
							}}
						>
							<div class="r-head">
								<span class="agency-badge">{result.agency || 'Unknown'}</span>
								<span class="match-score"
									>{(Math.max(0, 1 - result.distance) * 100).toFixed(1)}% Match</span
								>
							</div>
							<strong class="r-title">{result.title}</strong>
							<p class="r-excerpt">{result.excerpt}</p>
						</button>
					{/each}
				{:else if results && results.results.length === 0}
					<div class="empty-state">No relevant intelligence found.</div>
				{:else if !query}
					<div class="empty-state">Begin typing to run local vector search.</div>
				{/if}
			</div>
		</div>
	</div>
</Modal>

<style>
	.search-modal-wrap {
		display: flex;
		justify-content: center;
		align-items: flex-start;
		width: 100%;
		/* Modal's overlay centers its child by default (align-items: center); this wrap needs
		   to fill that full cross-axis itself so its own align-items: flex-start actually
		   positions the search bar near the top instead of dead-center. */
		height: 100%;
		padding-top: 10vh;
	}

	.search-modal {
		width: 640px;
		max-height: 70vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.search-bar {
		display: flex;
		align-items: center;
		padding: 16px 24px;
		border-bottom: 1px solid var(--color-border-subtle);
	}

	:global(.search-icon) {
		color: var(--color-text-secondary);
		margin-right: var(--space-3xl);
	}

	.search-bar input {
		flex: 1;
		background: transparent;
		border: none;
		color: var(--color-text-primary);
		font-size: var(--text-2xl);
		outline: none;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--color-border-subtle);
		border-top-color: var(--color-accent-primary);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	.search-results {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-3xl);
	}

	.results-meta {
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		margin-bottom: var(--space-xl);
		padding-left: var(--space-md);
	}

	.result-row {
		display: flex;
		flex-direction: column;
		width: 100%;
		text-align: left;
		padding: var(--space-3xl);
		border-radius: var(--radius-md);
		transition: background 0.2s;
	}

	.result-row:hover {
		background: rgba(255, 255, 255, 0.05);
	}

	.r-head {
		display: flex;
		justify-content: space-between;
		margin-bottom: var(--space-md);
	}

	.agency-badge {
		font-size: var(--text-xs);
		background: rgba(255, 255, 255, 0.1);
		padding: 2px 8px;
		border-radius: var(--radius-xs);
	}

	.match-score {
		font-size: var(--text-sm);
		color: var(--color-accent-primary);
		font-family: var(--font-mono);
	}

	.r-title {
		font-size: 15px;
		margin-bottom: var(--space-md);
	}

	.empty-state {
		padding: var(--space-8xl);
		text-align: center;
		color: var(--color-text-secondary);
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
