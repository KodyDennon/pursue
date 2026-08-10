<script lang="ts">
	let { assetUrl, title = 'Video Asset' } = $props<{ assetUrl: string; title?: string }>();
</script>

<div class="video-container glass-panel">
	<video controls autoplay src={assetUrl} preload="auto">
		<track kind="captions" />
		Your browser does not support the video tag.
	</video>
</div>

<style>
	.video-container {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		max-width: 960px;
		max-height: 80vh;
		background: #000;
		border-radius: var(--radius-lg, 16px);
		overflow: hidden;
		box-shadow: 0 20px 50px rgba(0, 0, 0, 0.7);
		border: 1px solid rgba(255, 255, 255, 0.1);
	}

	video {
		width: 100%;
		height: 100%;
		max-height: 80vh;
		object-fit: contain;
		background: #000;
		outline: none;
	}
</style>
