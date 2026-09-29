import { onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type UpdateChannel = 'stable' | 'beta'
export type LauncherUpdate = {
	available: boolean
	current_version: string
	version: string | null
	notes: string | null
	size_bytes: number | null
}
export type LauncherUpdateProgress = {
	phase: 'downloading' | 'installing' | 'restarting' | 'error'
	downloaded_bytes: number
	total_bytes: number | null
	bytes_per_second: number | null
	message?: string
}
export type LauncherUpdatePhase =
	| 'idle'
	| 'checking'
	| 'current'
	| 'available'
	| 'downloading'
	| 'installing'
	| 'restarting'
	| 'error'

const updateNumbers = new Intl.NumberFormat('ru', { maximumFractionDigits: 1 })
export function formatUpdateBytes(value: number | null | undefined): string {
	if (value === null || value === undefined || !Number.isFinite(value) || value < 0) return '—'
	if (value < 1024) return `${updateNumbers.format(value)} Б`
	if (value < 1048576) return `${updateNumbers.format(value / 1024)} КБ`
	return `${updateNumbers.format(value / 1048576)} МБ`
}

export function useLauncherUpdates() {
	const phase = ref<LauncherUpdatePhase>('idle')
	const candidate = ref<LauncherUpdate | null>(null)
	const candidateChannel = ref<UpdateChannel | null>(null)
	const progress = ref<LauncherUpdateProgress | null>(null)
	const error = ref('')
	let generation = 0
	let unlisten: UnlistenFn | undefined
	let mounted = true

	async function check(channel: UpdateChannel) {
		if (['downloading', 'installing', 'restarting'].includes(phase.value)) return
		const request = ++generation
		phase.value = 'checking'
		candidate.value = null
		candidateChannel.value = null
		progress.value = null
		error.value = ''
		try {
			const result = await invoke<LauncherUpdate>('launcher_check_update', { channel })
			if (!mounted || request !== generation) return
			if (result.available && result.version) {
				candidate.value = result
				candidateChannel.value = channel
				phase.value = 'available'
			} else phase.value = 'current'
		} catch (reason) {
			if (!mounted || request !== generation) return
			error.value = typeof reason === 'string' ? reason : 'Не удалось проверить обновления.'
			phase.value = 'error'
		}
	}

	async function install() {
		const channel = candidateChannel.value
		if (!channel || !candidate.value || phase.value !== 'available') return
		error.value = ''
		phase.value = 'downloading'
		progress.value = {
			phase: 'downloading',
			downloaded_bytes: 0,
			total_bytes: candidate.value.size_bytes,
			bytes_per_second: null,
		}
		try {
			await invoke('launcher_install_update', { channel })
			if (mounted) phase.value = 'restarting'
		} catch (reason) {
			if (!mounted) return
			error.value = typeof reason === 'string' ? reason : 'Не удалось установить обновление.'
			phase.value = 'error'
		}
	}

	function later() {
		if (phase.value !== 'available') return
		candidate.value = null
		candidateChannel.value = null
		phase.value = 'idle'
	}

	onMounted(() => {
		void listen<LauncherUpdateProgress>('launcher-update-progress', (event) => {
			if (!['downloading', 'installing', 'restarting'].includes(phase.value)) return
			progress.value = event.payload
			if (event.payload.phase === 'error') {
				error.value = event.payload.message || 'Не удалось установить обновление.'
				phase.value = 'error'
			} else phase.value = event.payload.phase
		})
			.then((stop) => {
				if (mounted) unlisten = stop
				else stop()
			})
			.catch(() => {})
	})
	onUnmounted(() => {
		mounted = false
		generation++
		unlisten?.()
	})

	return { phase, candidate, progress, error, check, install, later }
}
