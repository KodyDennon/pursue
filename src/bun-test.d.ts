declare module 'bun:test' {
	type TestFn = (name: string, fn: () => unknown | Promise<unknown>) => void;

	interface Matchers {
		toBe(expected: unknown): void;
		toThrow(expected?: RegExp | string): void;
		toBeNull(): void;
		not: Omit<Matchers, 'not'>;
	}

	export const describe: TestFn;
	export const test: TestFn;
	export function expect(value: unknown): Matchers;
}
