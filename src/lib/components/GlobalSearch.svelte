<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Search } from 'lucide-svelte';
	import { globalSearchOpen } from '$lib/store';
	import type { SearchResults } from '$lib/types';
	import { logger } from '$lib/logger';

	let query = $state('');
	let results = $state<SearchResults | null>(null);
	let loading = $state(false);
	let searchInput = $state<HTMLInputElement>();

	onMount(() => {
		logger.debug('[GlobalSearch] Mounted.');
		const handleKeydown = (e: KeyboardEvent) => {
			if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
				e.preventDefault();
				$globalSearchOpen = true;
			}
			if (e.key === 'Escape' && $globalSearchOpen) {
				$globalSearchOpen = false;
			}
		};
		window.addEventListener('keydown', handleKeydown);
		return () => window.removeEventListener('keydown', handleKeydown);
	});

	$effect(() => {
		if ($globalSearchOpen && searchInput) {
			setTimeout(() => searchInput?.focus(), 50);
		}
	});

	async function performSearch() {
		if (!query.trim()) {
			results = null;
			return;
		}
		loading = true;
		try {
			results = await invoke<SearchResults>('search', {
				request: { query: query.trim(), filters: {} }
			});
		} catch (e) {
			console.error(e);
		} finally {
			loading = false;
		}
	}
</script>

{#if $globalSearchOpen}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="search-overlay"
		onclick={(e) => {
			if (e.target === e.currentTarget) $globalSearchOpen = false;
		}}
	>
		<div class="search-modal glass-panel">
			<div class="search-bar">
				<Search size={20} class="search-icon" />
				<input
					bind:this={searchInput}
					bind:value={query}
					placeholder="Query intelligence index..."
					oninput={() => {
						// Debounce simple
						setTimeout(() => {
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
								// Select the record in the main view ideally, but for now just close
								$globalSearchOpen = false;
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
{/if}

<style>
	.search-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(4px);
		-webkit-backdrop-filter: blur(4px);
		z-index: 1000;
		display: flex;
		justify-content: center;
		align-items: flex-start;
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
		border-bottom: 1px solid var(--border-subtle);
	}

	:global(.search-icon) {
		color: var(--text-secondary);
		margin-right: 16px;
	}

	.search-bar input {
		flex: 1;
		background: transparent;
		border: none;
		color: var(--text-primary);
		font-size: 18px;
		outline: none;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--border-subtle);
		border-top-color: var(--accent-primary);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	.search-results {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
	}

	.results-meta {
		font-size: 11px;
		color: var(--text-secondary);
		text-transform: uppercase;
		margin-bottom: 12px;
		padding-left: 8px;
	}

	.result-row {
		display: flex;
		flex-direction: column;
		width: 100%;
		text-align: left;
		padding: 16px;
		border-radius: var(--radius-md);
		transition: background 0.2s;
	}

	.result-row:hover {
		background: rgba(255, 255, 255, 0.05);
	}

	.r-head {
		display: flex;
		justify-content: space-between;
		margin-bottom: 8px;
	}

	.agency-badge {
		font-size: 10px;
		background: rgba(255, 255, 255, 0.1);
		padding: 2px 8px;
		border-radius: 4px;
	}

	.match-score {
		font-size: 11px;
		color: var(--accent-primary);
		font-family: var(--font-mono);
	}

	.r-title {
		font-size: 15px;
		margin-bottom: 8px;
	}

	.empty-state {
		padding: 40px;
		text-align: center;
		color: var(--text-secondary);
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
