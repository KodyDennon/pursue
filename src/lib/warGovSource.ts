const WAR_GOV_ORIGIN = 'https://www.war.gov';
const WAR_GOV_UFO_CSV_PATH =
	/^\/Portals\/1\/Interactive\/\d{4}\/UFO\/[^"'<>]+\.csv(?:\?[^"'<>]*)?$/i;

function absolutizeWarGovUrl(value: string): string {
	const trimmed = value.trim();
	if (trimmed.startsWith('/')) return `${WAR_GOV_ORIGIN}${trimmed}`;
	return trimmed;
}

function isActiveUfoCsv(value: string): boolean {
	const absolute = absolutizeWarGovUrl(value);
	try {
		const url = new URL(absolute);
		return url.origin === WAR_GOV_ORIGIN && WAR_GOV_UFO_CSV_PATH.test(url.pathname + url.search);
	} catch {
		return false;
	}
}

export function discoverWarGovCsvUrl(html: string): string {
	const scriptCsvPattern = /const\s+csvUrl\s*=\s*["']([^"']+\.csv(?:\?[^"']*)?)["']/gi;
	for (const match of html.matchAll(scriptCsvPattern)) {
		const candidate = match[1];
		if (candidate && isActiveUfoCsv(candidate)) return absolutizeWarGovUrl(candidate);
	}

	throw new Error('Unable to find active WAR.gov UFO CSV in https://www.war.gov/UFO/');
}

export function validateWarGovCsv(csvText: string): void {
	const firstLine = csvText.replace(/^\uFEFF/, '').split(/\r?\n/, 1)[0] ?? '';
	const requiredHeaders = ['Release Date', 'Title', 'Type', 'Agency'];
	for (const header of requiredHeaders) {
		if (!firstLine.includes(header)) {
			throw new Error(`WAR.gov CSV is missing required header: ${header}`);
		}
	}
	if (firstLine.includes('<!DOCTYPE') || firstLine.includes('<html')) {
		throw new Error('WAR.gov CSV request returned HTML instead of CSV');
	}
}
