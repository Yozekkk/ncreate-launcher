import { invoke } from '@tauri-apps/api/core'

export type Loader = 'vanilla' | 'fabric' | 'forge' | 'neoforge' | 'quilt'
export const loaderNames: Record<Loader, string> = {
	vanilla: 'Vanilla',
	fabric: 'Fabric',
	forge: 'Forge',
	neoforge: 'NeoForge',
	quilt: 'Quilt',
}
export type Instance = {
	id: string
	name: string
	game_version: string
	loader: Loader
	loader_version: string | null
	kind: string
	edition: string | null
	manifest_version: string | null
	status: string
	memory_mb: number
	java_path: string | null
	directory: string
	icon?: string | null
	mod_count?: number
	last_played?: number | null
}
export type CreateInstance = {
	name: string
	game_version: string
	loader: Loader
	loader_version: string | null
	memory_mb: number
	java_path: string | null
}
export type InstalledContent = {
	id: string
	instance_id: string
	project_id: string | null
	version_id: string | null
	version_number?: string | null
	name: string
	kind: string
	path: string
	sha512: string
	enabled: boolean
	managed: boolean
	source_url: string | null
}
export type ContentKind = 'mod' | 'modpack'
export type SearchRequest = {
	query: string
	kind: ContentKind
	game_version: string | null
	loader: string | null
	category: string | null
	sort: string
	channel: 'stable' | 'beta'
	offset: number
	limit: number
}
export type SearchHit = {
	project_id: string
	slug: string
	title: string
	description: string
	author: string
	icon_url: string | null
	downloads: number
	categories: string[]
	versions: string[]
	date_modified: string
	project_type: string
}
export type SearchResult = { hits: SearchHit[]; offset: number; total_hits: number }
export type Project = {
	id: string
	slug: string
	title: string
	description: string
	body: string
	project_type: string
	icon_url: string | null
	downloads: number
	categories: string[]
	game_versions: string[]
	loaders: string[]
	updated: string
	team?: string
}
export type ProjectVersion = {
	id: string
	project_id: string
	name: string
	version_number: string
	version_type: 'release' | 'beta' | 'alpha'
	game_versions: string[]
	loaders: string[]
	date_published: string
	downloads: number
	files: { filename: string; size: number; primary: boolean; url: string }[]
	dependencies: { project_id: string | null; version_id: string | null; dependency_type: string }[]
}
export type GameVersion = { id: string; type: string; release_time?: string }
export type LoaderVersion = { id: string; stable: boolean }
export type ContentUpdate = {
	content_id: string
	current_version: string
	next_version: string
	name: string
}
export type Progress = {
	id: string
	instance_id: string | null
	operation: string
	phase: string
	completed: number
	total: number
	bytes_per_second?: number
	message: string
	cancellable: boolean
	error: string | null
}
export type RunningGame = { instance_id: string; pid: number; started_at: number; log_path: string }

export type EditionManifest = {
	schemaVersion: number
	id: string
	version: string
	minecraft: string
	loader: { kind: Loader; version: string | null }
	files: {
		path: string
		url: string
		sha256: string
		size: number
		required: boolean
		updatePolicy: 'managed_only' | 'preserve'
	}[]
	java: { major: number }
	memory: { minimumMb: number; recommendedMb: number; maximumMb: number }
	launch: { jvmArgs: string[]; gameArgs: string[] }
	servers: { name: string; address: string }[]
	releaseChannel: string
	changelog: string
}
export type EditionAvailability = {
	id: string
	channel: string
	available: boolean
	manifest: EditionManifest | null
	error: string | null
}
export type UpdatePlan = {
	instance_id: string
	from_version: string | null
	to_version: string
	added: string[]
	changed: string[]
	removed: string[]
	conflicts: string[]
	changelog: string
	download_bytes: number
}

export const gameApi = {
	editions: (channel: 'stable' | 'beta') =>
		invoke<EditionAvailability[]>('core_editions', { channel }),
	installEdition: (editionId: string, channel: 'stable' | 'beta') =>
		invoke<string>('core_install_edition', { editionId, channel }),
	checkEditionUpdate: (instanceId: string, channel: 'stable' | 'beta') =>
		invoke<UpdatePlan>('core_check_edition_update', { instanceId, channel }),
	applyEditionUpdate: (instanceId: string, channel: 'stable' | 'beta') =>
		invoke<string>('core_apply_edition_update', { instanceId, channel }),
	resumeEditionInstall: (instanceId: string, channel: 'stable' | 'beta') =>
		invoke<string>('core_resume_edition_install', { instanceId, channel }),
	rollbackAvailable: (instanceId: string) =>
		invoke<boolean>('core_rollback_available', { instanceId }),
	instances: () => invoke<Instance[]>('core_instances'),
	create: (request: CreateInstance) => invoke<Instance>('core_create_instance', { request }),
	edit: (instanceId: string, request: CreateInstance) =>
		invoke<Instance>('core_edit_instance', { instanceId, request }),
	remove: (instanceId: string) => invoke<void>('core_delete_instance', { instanceId }),
	duplicate: (instanceId: string, name: string) =>
		invoke<Instance>('core_duplicate_instance', { instanceId, name }),
	content: (instanceId: string) => invoke<InstalledContent[]>('core_content', { instanceId }),
	toggleContent: (contentId: string, enabled: boolean) =>
		invoke<void>('core_toggle_content', { contentId, enabled }),
	removeContent: (contentId: string) => invoke<void>('core_remove_content', { contentId }),
	checkUpdates: (instanceId: string) =>
		invoke<ContentUpdate[]>('core_check_updates', { instanceId }),
	updateContent: (instanceId: string, contentIds: string[]) =>
		invoke<string>('core_update_content', { instanceId, contentIds }),
	rollback: (instanceId: string) => invoke<void>('core_rollback', { instanceId }),
	search: (request: SearchRequest) => invoke<SearchResult>('core_search', { request }),
	project: (projectId: string) => invoke<Project>('core_project', { projectId }),
	versions: (projectId: string, gameVersion: string | null = null, loader: string | null = null) =>
		invoke<ProjectVersion[]>('core_versions', { projectId, gameVersion, loader }),
	installContent: (instanceId: string, projectId: string, versionId: string, kind: ContentKind) =>
		invoke<string>('core_install_content', {
			request: { instance_id: instanceId, project_id: projectId, version_id: versionId, kind },
		}),
	installModpack: (projectId: string, versionId: string, name: string) =>
		invoke<string>('core_install_modpack', { projectId, versionId, name }),
	installGame: (instanceId: string) => invoke<string>('core_install_game', { instanceId }),
	launch: (instanceId: string) => invoke<RunningGame>('core_launch', { instanceId }),
	stop: (instanceId: string) => invoke<void>('core_stop', { instanceId }),
	jobs: () => invoke<Progress[]>('core_jobs'),
	cancel: (jobId: string) => invoke<void>('core_cancel', { jobId }),
	gameVersions: () => invoke<GameVersion[]>('core_game_versions'),
	loaderVersions: (loader: Loader, gameVersion: string) =>
		invoke<LoaderVersion[]>('core_loader_versions', { loader, gameVersion }),
	categories: (kind: ContentKind) => invoke<string[]>('core_categories', { kind }),
	openFolder: (instanceId: string) => invoke<void>('core_open_folder', { instanceId }),
	pickIcon: (instanceId: string) => invoke<Instance | null>('pick_instance_icon', { instanceId }),
	pickImport: () => invoke<string | null>('pick_import_pack'),
	importPack: (path: string, name: string) => invoke<Instance>('core_import_pack', { path, name }),
	pickExport: (instanceId: string) => invoke<string | null>('pick_export_pack', { instanceId }),
	exportPack: (instanceId: string, path: string) =>
		invoke<void>('core_export_pack', { instanceId, path }),
}

export function compatibleVersions(
	versions: ProjectVersion[],
	instance: Instance | undefined,
): ProjectVersion[] {
	if (!instance) return []
	return versions.filter(
		(version) =>
			version.game_versions.includes(instance.game_version) &&
			version.loaders.includes(instance.loader),
	)
}
export function statusLabel(status: string): string {
	return (
		(
			{
				created: 'Нужна установка',
				ready: 'Готов к игре',
				running: 'Игра запущена',
				installing: 'Устанавливаем…',
				error: 'Нужна проверка',
			} as Record<string, string>
		)[status] || 'Локальная сборка'
	)
}
export function displayError(reason: unknown): string {
	return typeof reason === 'string'
		? reason
		: reason instanceof Error
			? reason.message
			: 'Не удалось выполнить действие. Попробуйте ещё раз.'
}

export function operationError(reason: string): string {
	if (/network request failed|request timed out|error sending request/i.test(reason))
		return 'Не удалось загрузить файлы. Проверьте подключение к сети и повторите действие.'
	if (/^(operation )?cancelled$/i.test(reason)) return 'Операция отменена.'
	return reason
}
