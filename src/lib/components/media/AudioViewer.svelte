<script lang="ts">
	import { Play, Pause, Volume2, VolumeX, Music, RotateCcw } from 'lucide-svelte';

	let { assetUrl, title = 'Audio Recording' } = $props<{
		assetUrl: string;
		title?: string;
	}>();

	let audioRef = $state<HTMLAudioElement | null>(null);
	let isPlaying = $state(false);
	let currentTime = $state(0);
	let duration = $state(0);
	let volume = $state(0.8);
	let isMuted = $state(false);

	function togglePlay() {
		if (!audioRef) return;
		if (isPlaying) {
			audioRef.pause();
		} else {
			audioRef.play();
		}
	}

	function handleTimeUpdate() {
		if (!audioRef) return;
		currentTime = audioRef.currentTime;
	}

	function handleLoadedMetadata() {
		if (!audioRef) return;
		duration = audioRef.duration;
	}

	function handleSeek(e: Event) {
		const target = e.target as HTMLInputElement;
		const val = parseFloat(target.value);
		if (audioRef) {
			audioRef.currentTime = val;
			currentTime = val;
		}
	}

	function handleVolumeChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const val = parseFloat(target.value);
		volume = val;
		if (audioRef) {
			audioRef.volume = val;
			isMuted = val === 0;
		}
	}

	function toggleMute() {
		if (!audioRef) return;
		isMuted = !isMuted;
		audioRef.muted = isMuted;
	}

	function restart() {
		if (!audioRef) return;
		audioRef.currentTime = 0;
		audioRef.play();
	}

	function formatTime(seconds: number): string {
		if (isNaN(seconds)) return '00:00';
		const mins = Math.floor(seconds / 60);
		const secs = Math.floor(seconds % 60);
		return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
	}
</script>

<div class="audio-viewer-card glass-panel">
	<audio
		bind:this={audioRef}
		src={assetUrl}
		onplay={() => (isPlaying = true)}
		onpause={() => (isPlaying = false)}
		ontimeupdate={handleTimeUpdate}
		onloadedmetadata={handleLoadedMetadata}
		onended={() => (isPlaying = false)}
	></audio>

	<div class="audio-visual-header">
		<div class="audio-icon-badge">
			<Music size={32} class="pulse-icon" />
		</div>
		<div class="audio-meta">
			<span class="audio-badge">AUDIO RECORDING</span>
			<h3 class="audio-title">{title}</h3>
		</div>
	</div>

	<!-- Animated Waveform Bars -->
	<div class="waveform-container" class:playing={isPlaying}>
		{#each Array.from({ length: 28 }, (_, i) => i) as barIndex (barIndex)}
			<div
				class="wave-bar"
				style="animation-delay: { (barIndex % 7) * 0.12 }s; height: { 15 + ((barIndex * 7) % 65) }%;"
			></div>
		{/each}
	</div>

	<!-- Seek Slider & Time -->
	<div class="scrubber-row">
		<span class="time-label">{formatTime(currentTime)}</span>
		<input
			type="range"
			class="seek-slider"
			min="0"
			max={duration || 100}
			step="0.1"
			value={currentTime}
			oninput={handleSeek}
		/>
		<span class="time-label">{formatTime(duration)}</span>
	</div>

	<!-- Playback Controls -->
	<div class="controls-row">
		<div class="left-controls">
			<button class="ctrl-btn" onclick={restart} title="Restart">
				<RotateCcw size={18} />
			</button>
		</div>

		<div class="center-controls">
			<button class="play-btn" onclick={togglePlay} title={isPlaying ? 'Pause' : 'Play'}>
				{#if isPlaying}
					<Pause size={24} />
				{:else}
					<Play size={24} style="margin-left: 3px;" />
				{/if}
			</button>
		</div>

		<div class="right-controls">
			<button class="ctrl-btn" onclick={toggleMute} title={isMuted ? 'Unmute' : 'Mute'}>
				{#if isMuted || volume === 0}
					<VolumeX size={18} />
				{:else}
					<Volume2 size={18} />
				{/if}
			</button>
			<input
				type="range"
				class="volume-slider"
				min="0"
				max="1"
				step="0.05"
				value={isMuted ? 0 : volume}
				oninput={handleVolumeChange}
			/>
		</div>
	</div>
</div>

<style>
	.audio-viewer-card {
		width: 90%;
		max-width: 580px;
		padding: 2rem;
		border-radius: var(--radius-lg, 16px);
		background: rgba(18, 22, 28, 0.85);
		border: 1px solid rgba(255, 255, 255, 0.1);
		box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		color: #fff;
	}

	.audio-visual-header {
		display: flex;
		align-items: center;
		gap: 1.25rem;
	}

	.audio-icon-badge {
		width: 56px;
		height: 56px;
		border-radius: 14px;
		background: linear-gradient(135deg, rgba(231, 196, 107, 0.2), rgba(231, 196, 107, 0.05));
		border: 1px solid rgba(231, 196, 107, 0.4);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-accent-primary, #e7c46b);
	}

	.audio-meta {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		overflow: hidden;
	}

	.audio-badge {
		font-size: 0.7rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		color: var(--color-accent-primary, #e7c46b);
	}

	.audio-title {
		margin: 0;
		font-size: 1.1rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.waveform-container {
		height: 70px;
		background: rgba(0, 0, 0, 0.4);
		border-radius: 10px;
		padding: 0 1rem;
		display: flex;
		align-items: center;
		justify-content: space-between;
		border: 1px solid rgba(255, 255, 255, 0.05);
	}

	.wave-bar {
		width: 4px;
		background: rgba(231, 196, 107, 0.3);
		border-radius: 2px;
		transition: height 0.2s ease;
	}

	.waveform-container.playing .wave-bar {
		background: var(--color-accent-primary, #e7c46b);
		animation: pulse-wave 0.8s ease-in-out infinite alternate;
	}

	@keyframes pulse-wave {
		0% {
			height: 15%;
		}
		100% {
			height: 85%;
		}
	}

	.scrubber-row {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.time-label {
		font-size: 0.8rem;
		font-family: var(--font-mono, monospace);
		color: rgba(255, 255, 255, 0.6);
		min-width: 42px;
	}

	.seek-slider {
		flex: 1;
		accent-color: var(--color-accent-primary, #e7c46b);
		height: 6px;
		border-radius: 3px;
		cursor: pointer;
	}

	.controls-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.left-controls,
	.right-controls {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		width: 140px;
	}

	.right-controls {
		justify-content: flex-end;
	}

	.center-controls {
		display: flex;
		justify-content: center;
	}

	.play-btn {
		width: 52px;
		height: 52px;
		border-radius: 50%;
		background: var(--color-accent-primary, #e7c46b);
		color: #12161c;
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition: transform 0.15s ease, box-shadow 0.15s ease;
		box-shadow: 0 4px 15px rgba(231, 196, 107, 0.3);
	}

	.play-btn:hover {
		transform: scale(1.06);
		box-shadow: 0 6px 20px rgba(231, 196, 107, 0.5);
	}

	.ctrl-btn {
		background: transparent;
		border: none;
		color: rgba(255, 255, 255, 0.7);
		cursor: pointer;
		padding: 6px;
		border-radius: 6px;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: color 0.15s ease;
	}

	.ctrl-btn:hover {
		color: #fff;
	}

	.volume-slider {
		width: 80px;
		accent-color: var(--color-accent-primary, #e7c46b);
		height: 4px;
		cursor: pointer;
	}
</style>
