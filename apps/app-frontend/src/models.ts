import { messages } from './i18n'

export type Account = {
	uuid: string
	nickname: string
	kind: 'microsoft' | 'offline'
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
}
export type Snapshot = { accounts: Account[]; settings: Settings; data_dir: string }
export type Skin = {
	provider: 'mojang' | 'ely_by' | 'fallback'
	head: string | null
	texture: string | null
	status: 'ready' | 'missing' | 'network_error'
}
export type EditionId = 'minimal' | 'standard' | 'ultra'
export type Edition = {
	id: EditionId
	name: string
	label: string
	description: string
	features: string[]
	recommended: boolean
	manifest: string | null
}
export const editions: Edition[] = [
	{
		id: 'minimal',
		name: 'Minimal',
		...messages.editions.minimal,
		recommended: false,
		manifest: null,
	},
	{
		id: 'standard',
		name: 'Standard',
		...messages.editions.standard,
		recommended: true,
		manifest: null,
	},
	{
		id: 'ultra',
		name: 'Ultra',
		...messages.editions.ultra,
		recommended: false,
		manifest: null,
	},
]
