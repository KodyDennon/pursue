import { describe, expect, test } from 'bun:test';
import capability from '../../src-tauri/capabilities/war-gov-resolver.json';

describe('Tauri capabilities', () => {
	test('WAR.gov resolver grants event IPC to the WAR.gov remote origin only', () => {
		expect(capability.windows.includes('war-gov-resolver')).toBe(true);
		expect(capability.local).toBe(false);
		expect(capability.permissions.includes('core:event:allow-emit')).toBe(true);
		expect(JSON.stringify(capability.remote?.urls)).toBe(JSON.stringify(['https://www.war.gov/*']));
	});
});
