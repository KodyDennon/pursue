<script lang="ts">
	import { ShieldAlert, DatabaseZap } from 'lucide-svelte';

	let { onSync } = $props<{
		onSync: () => void;
	}>();
</script>

<div class="empty-hero">
	<div class="hero-content">
		<div class="hero-icon">
			<ShieldAlert size={64} />
		</div>
		<h1>Command Center Offline</h1>
		<p>
			Your intelligence database is currently empty. Connect to an upstream source or ingest local
			evidence to begin analysis.
		</p>

		<div class="action-cards">
			<button class="action-card primary" onclick={onSync}>
				<DatabaseZap size={32} />
				<div class="ac-text">
					<strong>Sync WAR.gov</strong>
					<span>Download official UAP intelligence records from the public index.</span>
				</div>
			</button>
		</div>
	</div>
</div>

<style>
	.empty-hero {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		width: 100%;
		padding: 40px;
	}

	.hero-content {
		max-width: 600px;
		text-align: center;
	}

	.hero-icon {
		color: var(--text-secondary);
		margin-bottom: 24px;
		opacity: 0.5;
	}

	h1 {
		font-size: 32px;
		color: var(--text-primary);
		margin-bottom: 16px;
		letter-spacing: -0.02em;
	}

	p {
		font-size: 16px;
		color: var(--text-secondary);
		line-height: 1.6;
		margin-bottom: 48px;
	}

	.action-cards {
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 400px;
		margin: 0 auto;
	}

	.action-card {
		display: flex;
		align-items: center;
		gap: 20px;
		padding: 24px;
		background: var(--bg-surface-elevated);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		text-align: left;
		color: var(--text-primary);
		cursor: pointer;
		transition: all 0.2s;
	}

	.action-card:hover {
		border-color: var(--accent-primary);
		transform: translateY(-2px);
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
	}

	.action-card.primary {
		background: rgba(231, 196, 107, 0.05);
		border-color: rgba(231, 196, 107, 0.3);
	}

	.action-card.primary:hover {
		border-color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.1);
	}

	.action-card :global(svg) {
		color: var(--accent-primary);
		flex-shrink: 0;
	}

	.ac-text strong {
		display: block;
		font-size: 16px;
		margin-bottom: 4px;
	}

	.ac-text span {
		display: block;
		font-size: 13px;
		color: var(--text-secondary);
		line-height: 1.4;
	}
</style>
