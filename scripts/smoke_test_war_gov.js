import puppeteer from 'puppeteer';

const TEST_URL = 'https://www.war.gov/UFO/videos/ufo_video_1.mp4'; // Example synthetic URL or DVIDS URL

async function runSmokeTest() {
	console.log('Launching browser smoke test...');
	const browser = await puppeteer.launch({ headless: true });
	const page = await browser.newPage();

	console.log('Navigating to war.gov to establish origin and bypass standard WAF blocks...');
	await page.goto('https://www.war.gov/UFO/', { waitUntil: 'domcontentloaded', timeout: 30000 });

	console.log('Executing fetch from browser context...');
	const result = await page.evaluate(async (url) => {
		try {
			// Do a HEAD request or Range request to simulate chunked download
			const headers = new Headers();
			headers.set('Range', 'bytes=0-1024');
			
			const res = await fetch(url, { headers, cache: 'no-store' });
			return {
				ok: res.ok,
				status: res.status,
				statusText: res.statusText,
				headers: Object.fromEntries(res.headers.entries())
			};
		} catch (e) {
			return { error: e.toString() };
		}
	}, TEST_URL);

	if (result.error) {
		console.error('Smoke test FAILED:', result.error);
		process.exit(1);
	}

	console.log('Smoke test result:', result);

	if (result.status === 206 || result.status === 200) {
		console.log('Smoke test PASSED! Browser-based fetch bypasses WAF successfully.');
	} else if (result.status === 404) {
		console.log('Smoke test PASSED contextually (404 Not Found, but NOT 403 Forbidden).');
	} else {
		console.error(`Smoke test UNEXPECTED STATUS: ${result.status}`);
		process.exit(1);
	}

	await browser.close();
}

runSmokeTest().catch((e) => {
	console.error('Smoke test crashed:', e);
	process.exit(1);
});
