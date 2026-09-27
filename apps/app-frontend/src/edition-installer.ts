import { shallowRef } from 'vue'
import type { Edition, EditionId } from './models'

export type EditionInstallRequest = { editionId: EditionId; manifest: string }
export type EditionInstaller = (request: EditionInstallRequest) => Promise<void | boolean>

const installer = shallowRef<EditionInstaller | null>(null)

/** Register the future installation subsystem. Null disables all edition actions. */
export function configureEditionInstaller(handler: EditionInstaller | null) {
	installer.value = handler
}

export function canInstallEdition(edition: Edition): boolean {
	return edition.manifest !== null && edition.manifest.trim().length > 0 && installer.value !== null
}

/** Return false if unavailable or if the installer explicitly cancels the action. */
export async function installEdition(edition: Edition): Promise<boolean> {
	const handler = installer.value
	const manifest = edition.manifest
	if (!handler || !manifest || !manifest.trim()) return false
	return (await handler({ editionId: edition.id, manifest })) !== false
}
