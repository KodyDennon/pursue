import { describe, expect, test } from 'bun:test';
import { discoverWarGovCsvUrl, validateWarGovCsv } from './warGovSource';

describe('WAR.gov UFO source discovery', () => {
	test('uses the active page script CSV and ignores hidden legacy anchors', () => {
		const html = `
			<a style="display:none" href="/Portals/1/Interactive/2026/UFO/uap-release001.csv"></a>
			<script>
				const csvUrl = "/Portals/1/Interactive/2026/UFO/uap-data.csv";
				fetch(csvUrl).then(r => r.text());
			</script>
		`;

		expect(discoverWarGovCsvUrl(html)).toBe(
			'https://www.war.gov/Portals/1/Interactive/2026/UFO/uap-data.csv'
		);
	});

	test('rejects sandbox CSV references from unrelated WAR.gov modules', () => {
		const html = `
			<script>
				const csvUrl = "https://war.dod.afpims.mil/Portals/1/SANDBOXES/BEvans/testing-doc.csv";
			</script>
		`;

		expect(() => discoverWarGovCsvUrl(html)).toThrow(/active WAR.gov UFO CSV/i);
	});
});

describe('WAR.gov UFO CSV validation', () => {
	const validHeader =
		'Redaction,Release Date,Title,Type,Video Pairing,PDF Pairing,Description Blurb,DVIDS Video ID,Video Title,Agency,Incident Date,Incident Location,PDF | Image Link,Modal Image,Image Alt Text,Image VIRIN\n';

	test('accepts a CSV with all required headers', () => {
		expect(() => validateWarGovCsv(validHeader)).not.toThrow();
	});

	test('rejects a CSV missing a download-driving column, not just the basic ones', () => {
		const withoutDownloadColumns = validHeader.replace('PDF | Image Link,', '').replace('DVIDS Video ID,', '');
		expect(() => validateWarGovCsv(withoutDownloadColumns)).toThrow(
			/missing required header/i
		);
	});

	test('rejects an HTML error page returned instead of CSV', () => {
		expect(() => validateWarGovCsv('<!DOCTYPE html><html>...')).toThrow(
			/missing required header/i
		);
	});
});
