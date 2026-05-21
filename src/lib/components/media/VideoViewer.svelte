<script lang="ts">
	let { assetUrl } = $props<{ assetUrl: string }>();
</script>

<video controls autoplay src={assetUrl}>
	<track kind="captions" />
	Your browser does not support the video tag.
</video>

<style>
	video {
		max-width: 100%;
		max-height: 100%;
		background: #000;
	}
</style>
