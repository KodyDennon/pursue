import { describe, expect, test } from 'bun:test';
import { getRecordStatusClass } from './recordStatus';

describe('getRecordStatusClass', () => {
	test('maps each known analysis_status to its semantic class', () => {
		expect(getRecordStatusClass('completed')).toBe('ready');
		expect(getRecordStatusClass('indexed')).toBe('indexed');
		expect(getRecordStatusClass('synthesizing')).toBe('busy');
		expect(getRecordStatusClass('indexing')).toBe('pending');
		expect(getRecordStatusClass('extracting-foundation')).toBe('pending');
		expect(getRecordStatusClass('failed')).toBe('error');
	});

	test('falls back to unknown for unrecognized or missing status', () => {
		expect(getRecordStatusClass(null)).toBe('unknown');
		expect(getRecordStatusClass(undefined)).toBe('unknown');
		expect(getRecordStatusClass('some-future-status')).toBe('unknown');
	});
});
