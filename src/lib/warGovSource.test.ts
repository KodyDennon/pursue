import { describe, expect, test } from 'bun:test';
import { discoverWarGovCsvUrl } from './warGovSource';

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
