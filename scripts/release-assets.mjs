import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const tagPattern = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/
const repositoryPattern = /^[0-9A-Za-z_.-]+\/[0-9A-Za-z_.-]+$/

export function releaseNames(tag) {
	if (!tagPattern.test(tag)) throw new Error(`Invalid release tag: ${tag}`)
	const version = tag.slice(1)
	return {
		windows: `NCreate-Launcher-Setup-${version}.exe`,
		linux: `NCreate-Launcher-${version}.AppImage`,
		deb: `NCreate-Launcher-${version}-amd64.deb`,
		source: `NCreate-Launcher-${tag}-source.tar.gz`,
	}
}

function requiredFile(filename) {
	const stat = fs.statSync(filename)
	if (!stat.isFile() || stat.size === 0)
		throw new Error(`Empty or invalid release asset: ${filename}`)
	return stat
}

export function stageUpdateArtifacts({ tag, platform, bundleRoot, outputDir }) {
	const names = releaseNames(tag)
	const artifacts =
		platform === 'win32'
			? [['nsis', names.windows]]
			: platform === 'linux'
				? [
						['appimage', names.linux],
						['deb', names.deb],
					]
				: null
	if (!artifacts) throw new Error(`Unsupported release platform: ${platform}`)
	fs.mkdirSync(outputDir, { recursive: true })
	for (const [folder, name] of artifacts) {
		const file = path.join(bundleRoot, folder, name)
		requiredFile(file)
		fs.copyFileSync(file, path.join(outputDir, name))
		if (folder === 'deb') continue
		const signature = `${file}.sig`
		requiredFile(signature)
		fs.copyFileSync(signature, path.join(outputDir, `${name}.sig`))
	}
}

export function releaseChannel(tag, title) {
	releaseNames(tag)
	return tag.includes('-') || /\b(?:beta|alpha|rc|preview)\b|бета/i.test(title) ? 'beta' : 'stable'
}

export function createUpdaterMetadata({
	tag,
	repository,
	assetsDir,
	notes,
	publishedAt = new Date(),
}) {
	const names = releaseNames(tag)
	if (!repositoryPattern.test(repository))
		throw new Error(`Invalid GitHub repository: ${repository}`)
	const heading = /^# (.+)$/m.exec(notes)
	const title = heading?.[1] ?? `NCreate Launcher ${tag}`
	const channel = releaseChannel(tag, title)
	const platform = (name) => {
		const file = path.join(assetsDir, name)
		const signatureFile = path.join(assetsDir, `${name}.sig`)
		const size = requiredFile(file).size
		requiredFile(signatureFile)
		const signature = fs.readFileSync(signatureFile, 'utf8').trim()
		if (!signature) throw new Error(`Empty updater signature: ${signatureFile}`)
		return {
			url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(name)}`,
			signature,
			size,
			sha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex'),
		}
	}
	if (Number.isNaN(publishedAt.getTime())) throw new Error('Invalid publication date')
	return {
		version: tag.slice(1),
		channel,
		notes: heading ? notes.slice(heading.index + heading[0].length).trim() : notes.trim(),
		pub_date: publishedAt.toISOString(),
		platforms: {
			'linux-x86_64': platform(names.linux),
			'windows-x86_64': platform(names.windows),
		},
	}
}

const invokedAsScript =
	process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
if (invokedAsScript) {
	const [command] = process.argv.slice(2)
	const tag = process.env.RELEASE_TAG ?? process.env.GITHUB_REF_NAME
	const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
	const assetsDir = path.join(root, 'release-assets')
	if (command === 'stage') {
		stageUpdateArtifacts({
			tag,
			platform: process.platform,
			bundleRoot: path.join(root, 'target', 'release', 'bundle'),
			outputDir: assetsDir,
		})
	} else if (command === 'metadata') {
		const repository = process.env.GITHUB_REPOSITORY
		const notesFile = path.join(root, 'docs', 'releases', `${tag}.md`)
		const notes = fs.existsSync(notesFile)
			? fs.readFileSync(notesFile, 'utf8')
			: `# NCreate Launcher ${tag}\n\nWindows x64 and Linux x64 desktop packages are attached.\n`
		const metadata = createUpdaterMetadata({
			tag,
			repository,
			assetsDir,
			notes,
		})
		fs.writeFileSync(path.join(assetsDir, 'latest.json'), `${JSON.stringify(metadata, null, 2)}\n`)
	} else {
		throw new Error('Usage: node scripts/release-assets.mjs <stage|metadata>')
	}
}
