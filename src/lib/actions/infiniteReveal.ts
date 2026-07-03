/**
 * Svelte action for progressive/incremental rendering of large lists. Attach to a sentinel
 * element placed just after the currently-rendered slice of items; when it scrolls into view
 * (or is close to it, via `rootMargin`), `onReveal` fires so the caller can grow how many items
 * are actually mounted in the DOM.
 *
 * This is deliberately append-only (never un-mounts already-rendered items) rather than true
 * bidirectional virtualization: for CSS Grid layouts with a responsive/auto-fill column count,
 * computing which rows are on/off screen requires knowing the rendered column count, which
 * depends on live container width — getting that wrong risks content silently not rendering,
 * which is worse than the problem being solved. Append-only progressive reveal has no such
 * failure mode: worst case, it just doesn't grow the rendered count, which is exactly today's
 * (unvirtualized) behavior.
 */
export function infiniteReveal(node: Element, onReveal: () => void) {
	if (typeof IntersectionObserver === 'undefined') {
		// No IntersectionObserver support (or non-browser test environment) — leave the
		// caller's initial render count as the effective full render. Never throws.
		return {};
	}

	const observer = new IntersectionObserver(
		(entries) => {
			if (entries.some((entry) => entry.isIntersecting)) {
				onReveal();
			}
		},
		{ rootMargin: '600px 0px' }
	);
	observer.observe(node);

	return {
		destroy() {
			observer.disconnect();
		}
	};
}

/**
 * Pure logic behind infiniteReveal's growth: given how many items are currently rendered and
 * how many exist in total, how many should be rendered after the next reveal trigger. Kept
 * separate from the DOM/IntersectionObserver wiring above so it's unit-testable without a DOM
 * environment.
 */
export function nextRenderCount(currentlyRendered: number, total: number, batchSize: number): number {
	if (currentlyRendered >= total) return total;
	return Math.min(total, currentlyRendered + batchSize);
}
