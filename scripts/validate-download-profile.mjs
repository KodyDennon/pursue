import { readFile } from 'node:fs/promises';

const profile = JSON.parse(
	await readFile(new URL('../downloads/project.json', import.meta.url), 'utf8')
);

const fail = (message) => {
	throw new Error(`Invalid downloads/project.json: ${message}`);
};
const requireString = (value, path) => {
	if (typeof value !== 'string' || value.trim() === '') fail(`${path} must be a non-empty string`);
};
const requireHttps = (value, path) => {
	requireString(value, path);
	const url = new URL(value);
	if (url.protocol !== 'https:') fail(`${path} must use HTTPS`);
};
const requireContentList = (value, path) => {
	if (!Array.isArray(value) || value.length === 0) fail(`${path} must be a non-empty array`);
	value.forEach((item, index) => {
		requireString(item?.title, `${path}[${index}].title`);
		requireString(item?.body, `${path}[${index}].body`);
	});
};

if (profile.schema_version !== 1) fail('schema_version must be 1');
if (!/^[A-Z0-9][A-Z0-9_-]*$/.test(profile.slug ?? '')) fail('slug must be uppercase');
if (profile.route !== `/${profile.slug}`) fail('route must match /<slug>');
['name', 'short_name', 'status'].forEach((key) => requireString(profile[key], key));
['eyebrow', 'title', 'summary', 'action'].forEach((key) =>
	requireString(profile.card?.[key], `card.${key}`)
);
['coordinate', 'title', 'summary'].forEach((key) =>
	requireString(profile.hero?.[key], `hero.${key}`)
);
['source_url', 'repository_url', 'releases_url'].forEach((key) =>
	requireHttps(profile.project?.[key], `project.${key}`)
);
for (const key of [
	'download_index',
	'download_title',
	'capabilities_index',
	'capabilities_title',
	'capabilities_lead',
	'workflow_index',
	'workflow_title',
	'workflow_lead',
	'builds_index',
	'builds_title',
	'builds_lead',
	'support_index',
	'support_title',
	'support_lead',
	'integrity_index',
	'integrity_title',
	'integrity_lead'
]) {
	requireString(profile.sections?.[key], `sections.${key}`);
}
[
	'manifest_origin_url',
	'public_manifest_url',
	'updater_url',
	'stable_url_prefix'
].forEach((key) => requireHttps(profile.release?.[key], `release.${key}`));

const aliases = profile.release?.artifact_aliases;
if (!Array.isArray(aliases) || aliases.length === 0 || new Set(aliases).size !== aliases.length) {
	fail('release.artifact_aliases must contain unique aliases');
}
if (!Array.isArray(profile.platforms) || profile.platforms.length === 0) {
	fail('platforms must be a non-empty array');
}
const usedAliases = [];
for (const [platformIndex, platform] of profile.platforms.entries()) {
	['id', 'tab', 'readout'].forEach((key) =>
		requireString(platform?.[key], `platforms[${platformIndex}].${key}`)
	);
	if (!Array.isArray(platform.lanes) || platform.lanes.length === 0) {
		fail(`platforms[${platformIndex}].lanes must be a non-empty array`);
	}
	for (const [laneIndex, lane] of platform.lanes.entries()) {
		for (const key of ['alias', 'label', 'title', 'body', 'action', 'build_name', 'build_detail']) {
			requireString(lane?.[key], `platforms[${platformIndex}].lanes[${laneIndex}].${key}`);
		}
		usedAliases.push(lane.alias);
		if (lane.secondary_alias) usedAliases.push(lane.secondary_alias);
	}
}
if (new Set(usedAliases).size !== usedAliases.length) fail('lane aliases must not repeat');
if (usedAliases.length !== aliases.length || usedAliases.some((alias) => !aliases.includes(alias))) {
	fail('lane aliases must exactly match release.artifact_aliases');
}
requireContentList(profile.capabilities, 'capabilities');
requireContentList(profile.workflow, 'workflow');
requireContentList(profile.support_notes, 'support_notes');

console.log(`Validated download profile ${profile.slug} with ${aliases.length} release artifacts.`);
