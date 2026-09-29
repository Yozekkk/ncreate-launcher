import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, test } from 'node:test'
import {
	createUpdaterMetadata,
	releaseChannel,
	releaseNames,
	stageUpdateArtifacts,
} from './release-assets.mjs'

const temporary = []
afterEach(() => {
	for (const directory of temporary.splice(0))
		fs.rmSync(directory, { recursive: true, force: true })
})

function fixture() {
	const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ncreate-release-test-'))
	temporary.push(root)
	const names = releaseNames('v0.6.0')
	const bundles = path.join(root, 'bundle')
	const assets = path.join(root, 'assets')
	for (const [folder, name] of [
		['nsis', names.windows],
		['appimage', names.linux],
		['deb', names.deb],
	]) {
		const directory = path.join(bundles, folder)
		fs.mkdirSync(directory, { recursive: true })
		fs.writeFileSync(path.join(directory, name), `binary:${name}`)
		if (folder !== 'deb') fs.writeFileSync(path.join(directory, `${name}.sig`), `signed:${name}\n`)
	}
	return { root, names, bundles, assets }
}

test('release staging requires a signed updater bundle for each target', () => {
	const { names, bundles, assets } = fixture()
	stageUpdateArtifacts({
		tag: 'v0.6.0',
		platform: 'win32',
		bundleRoot: bundles,
		outputDir: assets,
	})
	stageUpdateArtifacts({
		tag: 'v0.6.0',
		platform: 'linux',
		bundleRoot: bundles,
		outputDir: assets,
	})
	assert.deepEqual(
		fs.readdirSync(assets).sort(),
		[names.windows, `${names.windows}.sig`, names.linux, `${names.linux}.sig`, names.deb].sort(),
	)
	fs.rmSync(path.join(bundles, 'appimage', `${names.linux}.sig`))
	assert.throws(
		() =>
			stageUpdateArtifacts({
				tag: 'v0.6.0',
				platform: 'linux',
				bundleRoot: bundles,
				outputDir: assets,
			}),
		/ENOENT/,
	)
})

test('metadata binds each OS to its exact artifact, signature, size and digest', () => {
	const { names, bundles, assets } = fixture()
	stageUpdateArtifacts({
		tag: 'v0.6.0',
		platform: 'win32',
		bundleRoot: bundles,
		outputDir: assets,
	})
	stageUpdateArtifacts({
		tag: 'v0.6.0',
		platform: 'linux',
		bundleRoot: bundles,
		outputDir: assets,
	})
	const metadata = createUpdaterMetadata({
		tag: 'v0.6.0',
		repository: 'Yozekkk/ncreate-launcher',
		assetsDir: assets,
		notes:
			'<p align="center">Русский</p>\n\n# NCreate Launcher v0.6.0 Beta\n\nLauncher improvements.\n',
		publishedAt: new Date('2026-01-02T03:04:05Z'),
	})
	assert.equal(metadata.version, '0.6.0')
	assert.equal(metadata.channel, 'beta')
	assert.equal(metadata.pub_date, '2026-01-02T03:04:05.000Z')
	assert.equal(metadata.notes, 'Launcher improvements.')
	for (const [target, name] of [
		['windows-x86_64', names.windows],
		['linux-x86_64', names.linux],
	]) {
		assert.deepEqual(metadata.platforms[target], {
			url: `https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/${name}`,
			signature: `signed:${name}`,
			size: Buffer.byteLength(`binary:${name}`),
			sha256: createHash('sha256').update(`binary:${name}`).digest('hex'),
		})
	}
})

test('stable and beta channels are classified from release title or prerelease tag', () => {
	assert.equal(releaseChannel('v0.6.0', 'NCreate Launcher v0.6.0'), 'stable')
	assert.equal(releaseChannel('v0.6.0', 'NCreate Launcher v0.6.0 Beta'), 'beta')
	assert.equal(releaseChannel('v0.6.0-beta.1', 'NCreate Launcher v0.6.0'), 'beta')
	assert.throws(() => releaseNames('v0.6.0/../secret'), /Invalid release tag/)
})
