declare module 'bun:test' {
	type TestFn = (name: string, fn: () => unknown | Promise<unknown>) => void;

	export const describe: TestFn;
	export const test: TestFn;
	export function expect(value: unknown): {
		toBe(expected: unknown): void;
		toThrow(expected?: RegExp | string): void;
	};
}
