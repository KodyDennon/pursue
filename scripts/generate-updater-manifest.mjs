import { writeFile } from 'node:fs/promises';

const token = process.env.GH_TOKEN || process.env.GITHUB_TOKEN;
const repository = process.env.GITHUB_REPOSITORY || 'KodyDennon/pursue';
const tag = process.env.TAG_NAME;

if (!token) throw new Error('GH_TOKEN or GITHUB_TOKEN is required');
if (!tag || !/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) {
	throw new Error(`TAG_NAME is not a supported release tag: ${tag ?? '<missing>'}`);
}
if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
	throw new Error(`GITHUB_REPOSITORY is invalid: ${repository}`);
}

const headers = {
	Accept: 'application/vnd.github+json',
	Authorization: `Bearer ${token}`,
	'X-GitHub-Api-Version': '2022-11-28',
	'User-Agent': 'pursue-release-manifest'
};

async function github(path, accept = headers.Accept) {
	const response = await fetch(`https://api.github.com${path}`, {
		headers: { ...headers, Accept: accept },
		signal: AbortSignal.timeout(30_000)
	});
	if (!response.ok) {
		throw new Error(`GitHub API ${response.status} for ${path}: ${await response.text()}`);
	}
	return response;
}

const release = await (await github(`/repos/${repository}/releases/tags/${encodeURIComponent(tag)}`)).json();
if (release.draft) throw new Error(`Refusing to publish updater metadata for draft release ${tag}`);
if (!Array.isArray(release.assets)) throw new Error('GitHub release response has no assets array');

function exactlyOne(label, predicate) {
	const matches = release.assets.filter((asset) => predicate(asset.name));
	if (matches.length !== 1) {
		throw new Error(`${label}: expected exactly one release asset, found ${matches.length}`);
	}
	return matches[0];
}

const lanes = [
	{
		target: 'macos-metal-aarch64',
		bundle: exactlyOne('Apple Silicon Metal updater', (name) => name.endsWith('.app.tar.gz'))
	},
	{
		target: 'windows-cuda-x86_64',
		bundle: exactlyOne(
			'Windows CUDA updater',
			(name) => name.endsWith('.msi.zip') && /(?:^|[-_])cuda(?:[-_.]|$)/i.test(name)
		)
	},
	{
		target: 'windows-directml-x86_64',
		bundle: exactlyOne(
			'Windows DirectML updater',
			(name) => name.endsWith('.nsis.zip') && !/(?:^|[-_])cuda(?:[-_.]|$)/i.test(name)
		)
	}
];

const platforms = {};
for (const lane of lanes) {
	const signatureAsset = exactlyOne(
		`${lane.target} signature`,
		(name) => name === `${lane.bundle.name}.sig`
	);
	if (!lane.bundle.browser_download_url?.startsWith('https://github.com/')) {
		throw new Error(`${lane.bundle.name} has an unexpected download URL`);
	}
	const signature = (
		await (
			await github(
				`/repos/${repository}/releases/assets/${signatureAsset.id}`,
				'application/octet-stream'
			)
		).text()
	).trim();
	if (signature.length < 50 || /\s/.test(signature)) {
		throw new Error(`${signatureAsset.name} is not a valid compact updater signature`);
	}
	platforms[lane.target] = {
		signature,
		url: lane.bundle.browser_download_url
	};
}

const manifest = {
	version: tag.slice(1),
	notes: `PURSUE Data Analyzer ${tag}`,
	pub_date: release.published_at || new Date().toISOString(),
	platforms
};

await writeFile('latest.json', `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx' });
console.log(`Generated signed updater manifest for ${Object.keys(platforms).length} release lanes.`);
