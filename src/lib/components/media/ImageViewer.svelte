<script lang="ts">
	import { ZoomIn, ZoomOut, RotateCw } from 'lucide-svelte';

	let { zoom = $bindable(), rotation = $bindable(), position = $bindable(), assetUrl, title } = $props<{
		zoom: number;
		rotation: number;
		position: { x: number; y: number };
		assetUrl: string;
		title: string;
	}>();
</script>

<div class="image-wrapper" style="transform: translate({position.x}px, {position.y}px)">
	<img
		src={assetUrl}
		alt={title}
		style="transform: scale({zoom}) rotate({rotation}deg)"
		draggable="false"
	/>
</div>

<style>
	.image-wrapper {
		transition: transform 0.1s ease-out;
	}

	img {
		max-width: 90%;
		max-height: 90%;
		object-fit: contain;
		user-select: none;
		pointer-events: none;
		transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
		box-shadow: 0 0 40px rgba(0, 0, 0, 0.5);
	}
</style>
