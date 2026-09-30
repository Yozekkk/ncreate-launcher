export type Account = {
	uuid: string
	nickname: string
	kind: 'microsoft' | 'offline' | 'ely_by'
	account_provider?: 'microsoft' | 'offline' | 'ely_by'
	game_identity?: { uuid: string; nickname: string }
	credential_reference?: { provider: 'microsoft' | 'ely_by'; key: string } | null
	active: boolean
	skin_provider: 'mojang' | 'ely_by' | 'fallback'
}
export type Settings = {
	locale: 'ru'
	theme: 'dark' | 'oled'
	animations: boolean
	blur: boolean
	reduced_motion: boolean
	memory_mb: number
	java_path: string
	game_directory: string
	auto_updates: boolean
	release_channel: 'stable' | 'beta'
}
export type Snapshot = { accounts: Account[]; settings: Settings; data_dir: string }
export type Skin = {
	provider: 'mojang' | 'ely_by' | 'fallback'
	head: string | null
	texture: string | null
	status: 'ready' | 'missing' | 'network_error'
}
export type EditionId = 'ncreate-server'
export type Edition = {
	id: EditionId
	name: string
	label: string
	description: string
	manifest: string | null
}
export const officialEdition: Edition = {
	id: 'ncreate-server',
	name: 'NCreate Server',
	label: 'Официальная серверная сборка',
	description: 'Официальная сборка для игры на сервере NCreate.',
	manifest: null,
}
