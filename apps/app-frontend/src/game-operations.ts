import { computed, onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { gameApi, displayError, operationError, type Progress } from './game-api'

export function isTerminalPhase(phase: string): boolean {
	return ['completed', 'failed', 'cancelled'].includes(phase)
}
export function operationPercent(job: Progress): number | null {
	return job.total > 0 ? Math.min(100, Math.max(0, (job.completed / job.total) * 100)) : null
}
export function useGameOperations(onFinish: (job: Progress) => void = () => {}) {
	const jobs = ref<Progress[]>([])
	const waiting = ref<string[]>([])
	const error = ref('')
	const cancelling = ref<string | null>(null)
	const active = computed(() => jobs.value.filter((job) => !isTerminalPhase(job.phase)))
	const finished = new Set<string>()
	let timer: ReturnType<typeof setTimeout> | undefined
	let unlisten: UnlistenFn | undefined
	let mounted = true
	let polling = false
	function accept(next: Progress[]) {
		jobs.value = next
		for (const job of next) {
			waiting.value = waiting.value.filter((id) => id !== job.id)
			if (isTerminalPhase(job.phase) && !finished.has(job.id)) {
				finished.add(job.id)
				if (job.error) error.value = operationError(job.error)
				onFinish(job)
			}
		}
		if (!active.value.length && !waiting.value.length) {
			clearTimeout(timer)
			timer = undefined
		}
	}
	async function poll() {
		if (!mounted || polling) return
		clearTimeout(timer)
		timer = undefined
		polling = true
		try {
			accept(await gameApi.jobs())
		} catch (reason) {
			error.value = displayError(reason)
			waiting.value = []
			jobs.value = []
		} finally {
			polling = false
			if (mounted && (active.value.length || waiting.value.length))
				timer = setTimeout(() => void poll(), 1000)
		}
	}
	async function track(jobId: string) {
		waiting.value = [...waiting.value, jobId]
		clearTimeout(timer)
		await poll()
	}
	async function cancel(jobId: string) {
		cancelling.value = jobId
		try {
			await gameApi.cancel(jobId)
			await poll()
		} catch (reason) {
			error.value = displayError(reason)
		} finally {
			cancelling.value = null
		}
	}
	onMounted(() => {
		void poll()
		void listen<Progress>('core-progress', (event) => {
			accept([...jobs.value.filter((job) => job.id !== event.payload.id), event.payload])
			if (active.value.length && !polling) {
				clearTimeout(timer)
				void poll()
			}
		})
			.then((stop) => {
				if (mounted) unlisten = stop
				else stop()
			})
			.catch(() => {})
	})
	onUnmounted(() => {
		mounted = false
		clearTimeout(timer)
		unlisten?.()
	})
	return {
		jobs,
		active,
		waiting,
		error,
		cancelling,
		track,
		cancel,
		clearError: () => {
			error.value = ''
		},
	}
}
export type GameOperations = ReturnType<typeof useGameOperations>
