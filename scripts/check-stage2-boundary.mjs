import fs from 'node:fs'
import path from 'node:path'

const config = JSON.parse(fs.readFileSync('apps/app/tauri.conf.json', 'utf8'))
const capability = JSON.parse(fs.readFileSync('apps/app/capabilities/main.json', 'utf8'))
if (config.identifier !== 'com.ncreate.launcher') throw new Error('Unexpected application identifier')
if (config.productName !== 'NCreate Launcher') throw new Error('Unexpected product identity')
if (config.plugins['deep-link'].desktop.schemes.join(',') !== 'ncreate') throw new Error('Unexpected deep-link scheme')
if (capability.windows.join(',') !== 'main' || capability.remote) throw new Error('Remote native access is forbidden')
if (capability.permissions.some(permission => /^(fs|shell|http|updater):/.test(typeof permission === 'string' ? permission : permission.identifier))) {
	throw new Error('Frontend must not receive broad filesystem/network/process/update access')
}
const csp = config.app.security.csp
if (!csp.includes("script-src 'self'") || !csp.includes("frame-src 'none'") || csp.includes('https://*.')) {
	throw new Error('Unexpected content security policy')
}
const forbidden = /https?:\/\/(?:[^\s"'`]*\.)?(?:posthog\.com|sentry\.io)|https?:\/\/(?:posthog\.modrinth\.com|archon\.modrinth\.com|shared-instances\.modrinth\.com)|launcher-files\.modrinth\.com\/updates\.json|modrinth\.com\/wrapper\/app-ads-cookie/
function inspect(directory) {
	for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
		if (['upstream', 'node_modules', 'dist'].includes(entry.name)) continue
		const file = path.join(directory, entry.name)
		if (entry.isDirectory()) inspect(file)
		else if (/\.(rs|ts|vue|json)$/.test(file) && forbidden.test(fs.readFileSync(file, 'utf8'))) {
			throw new Error(`Excluded product destination in ${file}`)
		}
	}
}
for (const directory of ['apps/app/src', 'apps/app-frontend/src', 'packages/app-lib/src', 'packages/launcher-core/src']) inspect(directory)
console.log('Stage 2 branding, capabilities, CSP and excluded network destinations checked')
