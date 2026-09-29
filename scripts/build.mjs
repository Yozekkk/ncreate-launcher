/** Builds the existing Tauri shell and normalizes public installer filenames. */
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const shell = path.join(repository, 'apps', 'app')
// linuxdeploy's bundled strip cannot read modern Arch/EndeavourOS RELR sections.
// Rust's release profile already strips our binary; keep dependency ELF files intact.
const environment = { ...process.env }
if (process.platform === 'linux') environment.NO_STRIP ??= 'true'
if (process.platform === 'linux' && !environment.TAURI_SIGNING_PRIVATE_KEY && !environment.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
	const signingDirectory = path.join(os.homedir(), '.local', 'share', 'ncreate-release-signing')
	const key = path.join(signingDirectory, 'updater.key')
	const password = path.join(signingDirectory, 'updater.password')
	if (fs.existsSync(key) && fs.existsSync(password)) {
		for (const file of [key, password]) {
			if (fs.statSync(file).mode & 0o077) throw new Error(`Updater signing file must be owner-only: ${file}`)
		}
		environment.TAURI_SIGNING_PRIVATE_KEY = key
		environment.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = fs.readFileSync(password, 'utf8').replace(/\r?\n$/, '')
	}
}
const result = spawnSync(process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', ['exec', 'tauri', 'build', ...process.argv.slice(2)], { cwd: shell, stdio: 'inherit', env: environment, shell: process.platform === 'win32' })
if (result.status !== 0) process.exit(result.status ?? 1)
const { version } = JSON.parse(fs.readFileSync(path.join(shell, 'tauri.conf.json'), 'utf8'))
const bundleFlag = process.argv.indexOf('--bundles')
const requestedBundles = bundleFlag === -1 ? null : new Set((process.argv[bundleFlag + 1] ?? '').split(','))
for (const [folder, extension, filename] of [
	['appimage', '.AppImage', `NCreate-Launcher-${version}.AppImage`],
	['nsis', '.exe', `NCreate-Launcher-Setup-${version}.exe`],
	['deb', '.deb', `NCreate-Launcher-${version}-amd64.deb`],
]) {
	if (requestedBundles && !requestedBundles.has(folder)) continue
	const directory = path.join(repository, 'target', 'release', 'bundle', folder)
	if (!fs.existsSync(directory)) {
		if (requestedBundles) throw new Error(`Missing ${folder} bundle directory`)
		continue
	}
	// Tauri uses "Name_version_arch"; old bundles must never overwrite this build.
	const artifacts = fs.readdirSync(directory).filter(name => name.endsWith(extension) && name.includes(`_${version}_`))
	if (artifacts.length !== 1) {
		if (requestedBundles || artifacts.length > 1) throw new Error(`Expected one ${version} ${folder} bundle, found ${artifacts.length}`)
		continue
	}
	const original = path.join(directory, artifacts[0])
	const signature = `${original}.sig`
	fs.renameSync(original, path.join(directory, filename))
	if (fs.existsSync(signature)) fs.renameSync(signature, path.join(directory, `${filename}.sig`))
}
