<script lang="ts">
	export let size: number = 28;
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 512 512"
	xmlns="http://www.w3.org/2000/svg"
	class={$$props.class}
>
	<defs>
		<radialGradient id="fortressBg" cx="50%" cy="50%" r="55%">
			<stop offset="0%" stop-color="#0f172a" />
			<stop offset="100%" stop-color="#020617" />
		</radialGradient>
		<linearGradient id="dataFlow" x1="0%" y1="0%" x2="100%" y2="100%">
			<stop offset="0%" stop-color="#22d3ee" />
			<stop offset="50%" stop-color="#06b6d4" />
			<stop offset="100%" stop-color="#0891b2" />
		</linearGradient>
		<filter id="ultraGlow" x="-50%" y="-50%" width="200%" height="200%">
			<feGaussianBlur stdDeviation="4" result="blur1" />
			<feGaussianBlur stdDeviation="10" result="blur2" />
			<feComposite in="SourceGraphic" in2="blur1" operator="over" />
			<feComposite in2="blur2" operator="over" />
		</filter>
		<clipPath id="hexClip">
			<polygon points="256,40 448,148 448,364 256,472 64,364 64,148" />
		</clipPath>
	</defs>
	<rect width="512" height="512" rx="128" fill="url(#fortressBg)" />

	<g transform="translate(256, 256)">
		<!-- Layered Translucent Architecture -->
		<g clip-path="url(#hexClip)" opacity="0.4">
			<path
				d="M -200 -100 L 200 -100 L 100 200 L -100 200 Z"
				fill="#1e293b"
				transform="rotate(30)"
			/>
			<path
				d="M -200 -100 L 200 -100 L 100 200 L -100 200 Z"
				fill="#334155"
				transform="rotate(150)"
			/>
		</g>

		<!-- Engineered Labyrinth (The Grid) -->
		<g fill="none" stroke="#1e293b" stroke-width="1">
			<circle r="220" />
			<circle r="160" stroke-dasharray="2 10" />
			<path d="M -256 0 L 256 0 M 0 -256 L 0 256" opacity="0.2" />
		</g>

		<!-- The Kinetic Data Path -->
		<g filter="url(#ultraGlow)">
			<path
				d="M -160 120 C -120 40 -40 160 0 0 C 40 -160 120 -40 160 -120"
				stroke="url(#dataFlow)"
				stroke-width="12"
				fill="none"
				stroke-linecap="round"
			/>
			<!-- Active Signal Nodes -->
			<circle cx="0" cy="0" r="16" fill="#fff" />
			<circle cx="-160" cy="120" r="6" fill="#22d3ee" />
			<circle cx="160" cy="-120" r="6" fill="#22d3ee" />
		</g>

		<!-- Holographic Metadata -->
		<g fill="#0891b2" font-family="monospace" font-size="10" letter-spacing="3" opacity="0.8">
			<text x="-180" y="-180">SYS_AUTH: P-90</text>
			<text x="60" y="200">EXTRACT_v4</text>
			<path d="M -190 -170 L -190 -195 L -160 -195" fill="none" stroke="#0891b2" stroke-width="1" />
			<path d="M 190 190 L 190 215 L 160 215" fill="none" stroke="#0891b2" stroke-width="1" />
		</g>
	</g>
</svg>
