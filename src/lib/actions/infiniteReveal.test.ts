import { describe, expect, test } from 'bun:test';
import { nextRenderCount } from './infiniteReveal';

describe('nextRenderCount', () => {
	test('grows by batchSize, capped at total', () => {
		expect(nextRenderCount(60, 294, 60)).toBe(120);
		expect(nextRenderCount(280, 294, 60)).toBe(294);
	});

	test('is a no-op once everything is already rendered', () => {
		expect(nextRenderCount(294, 294, 60)).toBe(294);
		expect(nextRenderCount(300, 294, 60)).toBe(294);
	});

	test('handles an empty list', () => {
		expect(nextRenderCount(0, 0, 60)).toBe(0);
	});
});
