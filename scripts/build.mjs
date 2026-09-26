/** Builds the existing Tauri shell and normalizes public installer filenames. */
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const shell = path.join(repository, 'apps', 'app')
// linuxdeploy's bundled strip cannot read modern Arch/EndeavourOS RELR sections.
// Rust's release profile already strips our binary; keep dependency ELF files intact.
const environment = { ...process.env }
if (process.platform === 'linux') environment.NO_STRIP ??= 'true'
const result = spawnSync(process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', ['exec', 'tauri', 'build', ...process.argv.slice(2)], { cwd: shell, stdio: 'inherit', env: environment, shell: process.platform === 'win32' })
if (result.status !== 0) process.exit(result.status ?? 1)
const { version } = JSON.parse(fs.readFileSync(path.join(shell, 'tauri.conf.json'), 'utf8'))
for (const [folder, extension, filename] of [
	['appimage', '.AppImage', `NCreate-Launcher-${version}.AppImage`],
	['nsis', '.exe', `NCreate-Launcher-Setup-${version}.exe`],
	['deb', '.deb', `NCreate-Launcher-${version}-amd64.deb`],
]) {
	const directory = path.join(repository, 'target', 'release', 'bundle', folder)
	if (!fs.existsSync(directory)) continue
	for (const artifact of fs.readdirSync(directory).filter(name => name.endsWith(extension))) {
		if (artifact !== filename) fs.renameSync(path.join(directory, artifact), path.join(directory, filename))
	}
}
