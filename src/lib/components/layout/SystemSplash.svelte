<div class="system-splash">
	<div class="splash-content">
		<div class="loader-spinner"></div>
		<h2>INTELLIGENCE OS INITIALIZING</h2>
		<p>Syncing local evidence vault and neural models...</p>
		<div class="boot-log">
			<span>[SYSTEM] Mounting secure database...</span>
			<span>[SYSTEM] Initializing vector search engine...</span>
			<span>[SYSTEM] Loading AARO official source records...</span>
		</div>
	</div>
</div>

<style>
	.system-splash {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: #000;
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.splash-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
		text-align: center;
	}

	h2 {
		font-size: 24px;
		color: var(--text-primary);
		margin: 0;
	}

	p {
		color: var(--text-secondary);
		margin: 0;
	}

	.loader-spinner {
		width: 40px;
		height: 40px;
		border: 3px solid rgba(231, 196, 107, 0.1);
		border-top: 3px solid var(--accent-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin-bottom: 24px;
	}

	@keyframes spin {
		0% { transform: rotate(0deg); }
		100% { transform: rotate(360deg); }
	}

	.boot-log {
		margin-top: 24px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--accent-primary);
		opacity: 0.7;
		text-align: left;
		width: 300px;
	}
</style>
