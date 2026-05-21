<div class="ambient-background">
	<div class="ambient-blob b1"></div>
	<div class="ambient-blob b2"></div>
	<div class="ambient-blob b3"></div>
</div>

<style>
	.ambient-background {
		position: fixed;
		inset: 0;
		z-index: -1;
		background: #000;
		overflow: hidden;
	}

	.ambient-blob {
		position: absolute;
		border-radius: 50%;
		filter: blur(100px);
		opacity: 0.4;
		animation: ambient-drift 20s infinite alternate cubic-bezier(0.4, 0, 0.2, 1);
	}

	.b1 {
		top: -10%; left: -10%;
		width: 50vw; height: 50vw;
		background: rgba(231, 196, 107, 0.15); /* Accent Primary */
		animation-delay: 0s;
	}
	.b2 {
		bottom: -20%; right: -10%;
		width: 60vw; height: 60vw;
		background: rgba(77, 243, 169, 0.1); /* Accent Success */
		animation-delay: -5s;
		animation-duration: 25s;
	}
	.b3 {
		top: 40%; left: 60%;
		width: 40vw; height: 40vw;
		background: rgba(243, 77, 77, 0.08); /* Accent Danger */
		animation-delay: -10s;
		animation-duration: 30s;
	}

	@keyframes ambient-drift {
		0% {
			transform: translate(0, 0) scale(1);
		}
		50% {
			transform: translate(5vw, 5vh) scale(1.1);
		}
		100% {
			transform: translate(-3vw, -2vh) scale(0.95);
		}
	}
</style>
