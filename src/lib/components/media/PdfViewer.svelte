<script lang="ts">
	let { assetUrl } = $props<{ assetUrl: string }>();
</script>

<iframe src="{assetUrl}#toolbar=0" title="PDF Viewer"></iframe>

<style>
	iframe {
		width: 100%;
		height: 100%;
		border: none;
		background: #fff;
	}
</style>
