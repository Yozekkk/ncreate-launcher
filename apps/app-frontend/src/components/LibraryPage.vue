<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { operationTitle, operationMessage } from '../operation-labels'
import AppIcon from './AppIcon.vue'
import OperationPanel from './OperationPanel.vue'
import {
	gameApi,
	displayError,
	loaderNames,
	statusLabel,
	type CreateInstance,
	type Instance,
	type InstalledContent,
	type GameVersion,
	type LoaderVersion,
	type Loader,
	type ContentUpdate,
	type UpdatePlan,
} from '../game-api'
import { isTerminalPhase, operationPercent, type GameOperations } from '../game-operations'
import type { Settings } from '../models'

const props = defineProps<{
	detail: string | null
	settings: Settings
	accountName: string | null
	operations: GameOperations
}>()
const instances = ref<Instance[]>([])
const mods = ref<InstalledContent[]>([])
const updates = ref<ContentUpdate[]>([])
const loading = ref(true)
const busy = ref(false)
const error = ref('')
const notice = ref('')
const createDialog = ref<HTMLDialogElement | null>(null)
const manageDialog = ref<HTMLDialogElement | null>(null)
const updateDialog = ref<HTMLDialogElement | null>(null)
const modPluralRules = new Intl.PluralRules('ru')
function modCountLabel(count: number): string {
	const labels: Record<string, string> = { one: 'мод', few: 'мода', many: 'модов', other: 'модов' }
	return `${count} ${labels[modPluralRules.select(count)]}`
}
const removeModDialog = ref<HTMLDialogElement | null>(null)
const editionUpdateDialog = ref<HTMLDialogElement | null>(null)
const editionPlan = ref<UpdatePlan | null>(null)
type OfficialUpdateStatus = {
	phase: 'checking' | 'current' | 'available' | 'error'
	version?: string
}
const officialUpdateStatus = ref<Record<string, OfficialUpdateStatus>>({})
const editionHasChanges = computed(
	() =>
		!!editionPlan.value &&
		!!(
			editionPlan.value.added.length +
			editionPlan.value.changed.length +
			editionPlan.value.removed.length
		),
)
const rollbackDialog = ref<HTMLDialogElement | null>(null)
const rollbackAvailable = ref(false)
const nameInput = ref<HTMLInputElement | null>(null)
const createName = ref('')
const gameVersion = ref('')
const loader = ref<Loader>('vanilla')
const loaderVersion = ref('')
const versions = ref<GameVersion[]>([])
const loaderVersions = ref<LoaderVersion[]>([])
const metadataLoading = ref(false)
const metadataError = ref('')
const includeSnapshots = ref(false)
const managed = ref<Instance | null>(null)
const manageName = ref('')
const manageMemory = ref(4096)
const manageJava = ref('')
const manageAction = ref<'rename' | 'duplicate' | 'delete' | 'configure'>('rename')
const removingMod = ref<InstalledContent | null>(null)
const selectedUpdateIds = ref<string[]>([])
const modFilter = ref('')
let metadataGeneration = 0
let contentGeneration = 0
let mounted = true
let gameTimer: ReturnType<typeof setTimeout> | undefined
let instanceGeneration = 0
let rollbackGeneration = 0
let officialUpdateGeneration = 0
const current = computed(() => instances.value.find((instance) => instance.id === props.detail))
const visibleVersions = computed(() =>
	versions.value.filter((version) => includeSnapshots.value || version.type === 'release'),
)
const visibleMods = computed(() =>
	mods.value.filter((mod) => mod.name.toLowerCase().includes(modFilter.value.toLowerCase())),
)
const libraryInstances = computed(() =>
	[...instances.value].sort(
		(a, b) => Number(b.kind === 'official') - Number(a.kind === 'official'),
	),
)
const localJobs = computed(() =>
	props.operations.active.value.filter(
		(job) => !current.value || job.instance_id === current.value.id,
	),
)
const instanceBusy = (id: string) =>
	props.operations.active.value.some((job) => job.instance_id === id)
const dates = new Intl.DateTimeFormat('ru', { day: 'numeric', month: 'short', year: 'numeric' })
const lastPlayed = (instance: Instance) =>
	instance.last_played ? dates.format(new Date(instance.last_played * 1000)) : 'Ещё не запускалась'
async function loadRollbackAvailability() {
	const instance = current.value
	const generation = ++rollbackGeneration
	rollbackAvailable.value = false
	if (!instance) return
	try {
		const available = await gameApi.rollbackAvailable(instance.id)
		if (mounted && generation === rollbackGeneration) rollbackAvailable.value = available
	} catch {
		if (mounted && generation === rollbackGeneration) rollbackAvailable.value = false
	}
}
async function loadOfficialUpdateStatus() {
	const generation = ++officialUpdateGeneration
	const official = instances.value.filter((instance) => instance.kind === 'official')
	officialUpdateStatus.value = Object.fromEntries(
		official.map((instance) => [instance.id, { phase: 'checking' }]),
	)
	await Promise.all(
		official.map(async (instance) => {
			let status: OfficialUpdateStatus
			try {
				const plan = await gameApi.checkEditionUpdate(instance.id, props.settings.release_channel)
				status =
					plan.from_version !== plan.to_version ||
					plan.added.length + plan.changed.length + plan.removed.length > 0
						? { phase: 'available', version: plan.to_version }
						: { phase: 'current' }
			} catch {
				status = { phase: 'error' }
			}
			if (mounted && generation === officialUpdateGeneration)
				officialUpdateStatus.value = { ...officialUpdateStatus.value, [instance.id]: status }
		}),
	)
}
function requestFor(instance: Instance, name: string): CreateInstance {
	return {
		name,
		game_version: instance.game_version,
		loader: instance.loader,
		loader_version: instance.loader_version,
		memory_mb: instance.memory_mb,
		java_path: instance.java_path,
	}
}
function dismissError() {
	error.value = ''
	props.operations.clearError()
}
function showRemoveMod(mod: InstalledContent) {
	removingMod.value = mod
	removeModDialog.value?.showModal()
}
function scheduleGameRefresh() {
	clearTimeout(gameTimer)
	gameTimer = undefined
	if (mounted && instances.value.some((instance) => instance.status === 'running'))
		gameTimer = setTimeout(() => void loadInstances(true), 3000)
}
async function loadInstances(quiet = false) {
	const generation = ++instanceGeneration
	if (!quiet) {
		loading.value = true
		error.value = ''
	}
	let loaded = false
	try {
		const result = await gameApi.instances()
		if (!mounted || generation !== instanceGeneration) return
		instances.value = result
		loaded = true
		if (!quiet) {
			void loadRollbackAvailability()
			void loadOfficialUpdateStatus()
		}
		if (!quiet && props.detail) await loadContent()
	} catch (reason) {
		if (mounted && generation === instanceGeneration) error.value = displayError(reason)
	} finally {
		if (mounted && generation === instanceGeneration) {
			loading.value = false
			if (loaded) scheduleGameRefresh()
			else clearTimeout(gameTimer)
		}
	}
}
async function loadContent() {
	const id = props.detail
	const generation = ++contentGeneration
	mods.value = []
	updates.value = []
	if (!id) return
	try {
		const result = await gameApi.content(id)
		if (generation === contentGeneration) mods.value = result
	} catch (reason) {
		if (generation === contentGeneration) error.value = displayError(reason)
	}
}
async function operation(action: () => Promise<unknown>, success = '', reload = true) {
	busy.value = true
	error.value = ''
	try {
		await action()
		if (success) notice.value = success
		if (reload) await loadInstances()
		return true
	} catch (reason) {
		error.value = displayError(reason)
		return false
	} finally {
		busy.value = false
	}
}
async function loadMetadata() {
	metadataLoading.value = true
	metadataError.value = ''
	try {
		versions.value = await gameApi.gameVersions()
		gameVersion.value = versions.value.find((version) => version.type === 'release')?.id || ''
	} catch (reason) {
		metadataError.value = displayError(reason)
	} finally {
		metadataLoading.value = false
	}
}
async function loadLoaderVersions() {
	const generation = ++metadataGeneration
	loaderVersions.value = []
	loaderVersion.value = ''
	if (loader.value === 'vanilla' || !gameVersion.value) {
		metadataLoading.value = false
		metadataError.value = ''
		return
	}
	metadataLoading.value = true
	metadataError.value = ''
	try {
		const result = await gameApi.loaderVersions(loader.value, gameVersion.value)
		if (generation === metadataGeneration) {
			loaderVersions.value = result
			loaderVersion.value = result.find((version) => version.stable)?.id || result[0]?.id || ''
			if (!result.length)
				metadataError.value =
					'Для этой версии Minecraft загрузчик недоступен. Выберите другую версию.'
		}
	} catch (reason) {
		if (generation === metadataGeneration) metadataError.value = displayError(reason)
	} finally {
		if (generation === metadataGeneration) metadataLoading.value = false
	}
}
async function showCreate() {
	createName.value = ''
	loader.value = 'vanilla'
	metadataError.value = ''
	error.value = ''
	createDialog.value?.showModal()
	await nextTick()
	nameInput.value?.focus()
	await loadMetadata()
}
async function createInstance() {
	if (
		!createName.value.trim() ||
		!gameVersion.value ||
		(loader.value !== 'vanilla' && !loaderVersion.value)
	) {
		metadataError.value = 'Укажите название, версию Minecraft и доступную версию загрузчика.'
		return
	}
	busy.value = true
	metadataError.value = ''
	try {
		const result = await gameApi.create({
			name: createName.value.trim(),
			game_version: gameVersion.value,
			loader: loader.value,
			loader_version: loader.value === 'vanilla' ? null : loaderVersion.value,
			memory_mb: props.settings.memory_mb,
			java_path: props.settings.java_path || null,
		})
		createDialog.value?.close()
		await loadInstances()
		location.hash = `/library/${result.id}`
	} catch (reason) {
		metadataError.value = displayError(reason)
	} finally {
		busy.value = false
	}
}
function showManage(instance: Instance, action: 'rename' | 'duplicate' | 'delete' | 'configure') {
	managed.value = instance
	manageName.value = action === 'duplicate' ? `${instance.name} — копия` : instance.name
	manageMemory.value = instance.memory_mb
	manageJava.value = instance.java_path || ''
	manageAction.value = action
	error.value = ''
	manageDialog.value?.showModal()
}
async function manageInstance() {
	const instance = managed.value
	if (!instance) return
	const success = await operation(
		async () => {
			if (manageAction.value === 'delete') {
				await gameApi.remove(instance.id)
				if (props.detail === instance.id) location.hash = '/library'
			} else if (manageAction.value === 'duplicate')
				await gameApi.duplicate(instance.id, manageName.value.trim())
			else if (manageAction.value === 'configure')
				await gameApi.edit(instance.id, {
					...requestFor(instance, instance.name),
					memory_mb: manageMemory.value,
					java_path: manageJava.value.trim() || null,
				})
			else await gameApi.edit(instance.id, requestFor(instance, manageName.value.trim()))
		},
		manageAction.value === 'delete' ? 'Сборка удалена из библиотеки' : 'Библиотека обновлена',
	)
	if (success) manageDialog.value?.close()
}
async function installOrPlay(instance: Instance) {
	if (instance.status === 'running') {
		await operation(() => gameApi.stop(instance.id), 'Игра остановлена')
		return
	}
	if (instance.status !== 'ready') {
		await operation(async () => props.operations.track(await gameApi.installGame(instance.id)))
		return
	}
	if (!props.accountName) {
		error.value = 'Добавьте или выберите активный аккаунт на странице «Аккаунты».'
		return
	}
	await operation(() => gameApi.launch(instance.id), 'Minecraft запущен')
}
async function importPack() {
	await operation(async () => {
		const path = await gameApi.pickImport()
		if (!path) return
		const result = await gameApi.importPack(path, '')
		location.hash = `/library/${result.id}`
	})
}
async function exportPack(instance: Instance) {
	await operation(async () => {
		const path = await gameApi.pickExport(instance.id)
		if (path) {
			await gameApi.exportPack(instance.id, path)
			notice.value = 'Экспорт завершён'
		}
	})
}
async function chooseIcon(instance: Instance) {
	await operation(() => gameApi.pickIcon(instance.id))
}
async function checkUpdates() {
	const instance = current.value
	if (instance)
		await operation(
			async () => {
				updates.value = await gameApi.checkUpdates(instance.id)
				await loadContent()
			},
			'',
			false,
		)
	if (!updates.value.length && !error.value) notice.value = 'Совместимых обновлений не найдено'
}
function showUpdates(ids: string[]) {
	selectedUpdateIds.value = ids
	error.value = ''
	updateDialog.value?.showModal()
}
async function applyUpdates() {
	if (!current.value) return
	const id = current.value.id
	if (
		await operation(async () =>
			props.operations.track(await gameApi.updateContent(id, selectedUpdateIds.value)),
		)
	)
		updateDialog.value?.close()
}
async function toggleMod(mod: InstalledContent) {
	if (await operation(() => gameApi.toggleContent(mod.id, !mod.enabled))) await loadContent()
}
async function removeMod() {
	if (
		removingMod.value &&
		(await operation(() => gameApi.removeContent(removingMod.value!.id), 'Мод удалён'))
	)
		removeModDialog.value?.close()
}
async function checkOfficialUpdate() {
	const instance = current.value
	if (!instance) return
	editionPlan.value = null
	if (
		await operation(
			async () => {
				editionPlan.value = await gameApi.checkEditionUpdate(
					instance.id,
					props.settings.release_channel,
				)
			},
			'',
			false,
		)
	)
		editionUpdateDialog.value?.showModal()
}
async function applyOfficialUpdate() {
	const instance = current.value
	if (!instance || !editionPlan.value) return
	if (
		await operation(async () =>
			props.operations.track(
				await gameApi.applyEditionUpdate(instance.id, props.settings.release_channel),
			),
		)
	)
		editionUpdateDialog.value?.close()
}
async function rollback() {
	if (
		current.value &&
		(await operation(
			() => gameApi.rollback(current.value!.id),
			'Предыдущее состояние восстановлено',
		))
	)
		rollbackDialog.value?.close()
}
watch([loader, gameVersion], () => void loadLoaderVersions())
watch(
	() => props.settings.release_channel,
	() => void loadOfficialUpdateStatus(),
)
watch(
	() => props.detail,
	() => {
		error.value = ''
		notice.value = ''
		void loadContent()
		void loadRollbackAvailability()
	},
)
watch(
	() =>
		props.operations.jobs.value
			.filter((job) => isTerminalPhase(job.phase))
			.map((job) => job.id)
			.join(','),
	() => void loadInstances(),
)
onMounted(() => void loadInstances())
onUnmounted(() => {
	mounted = false
	instanceGeneration++
	rollbackGeneration++
	officialUpdateGeneration++
	contentGeneration++
	clearTimeout(gameTimer)
})
</script>

<template>
	<section class="page library-page">
		<div class="page-heading">
			<div>
				<p class="eyebrow">ТВОИ МИРЫ. ТВОИ СБОРКИ.</p>
				<h1>
					{{ current ? current.name : 'Библиотека'
					}}<span v-if="!current" class="heading-count">{{ libraryInstances.length }}</span>
				</h1>
				<p class="page-intro">
					{{
						current
							? `Minecraft ${current.game_version} · ${loaderNames[current.loader]}${current.loader_version ? ` ${current.loader_version}` : ''}`
							: 'Создавай свои сборки и возвращайся в любимые миры.'
					}}
				</p>
			</div>
			<div class="heading-actions">
				<button class="button secondary" :disabled="busy" @click="importPack">
					<AppIcon name="folder" :size="18" />Импорт .mrpack</button
				><button class="button primary" :disabled="busy" @click="showCreate">
					<AppIcon name="plus" :size="18" />Создать сборку
				</button>
			</div>
		</div>
		<div v-if="error || operations.error.value" class="banner error inline-banner" role="alert">
			{{ error || operations.error.value
			}}<button class="icon-button" aria-label="Закрыть ошибку" @click="dismissError">
				<AppIcon name="close" :size="16" />
			</button>
		</div>
		<div v-if="notice" class="banner inline-banner" role="status">
			{{ notice
			}}<button class="icon-button" aria-label="Закрыть сообщение" @click="notice = ''">
				<AppIcon name="close" :size="16" />
			</button>
		</div>
		<OperationPanel
			v-for="job in localJobs"
			:key="job.id"
			:title="operationTitle(job.operation)"
			:message="operationMessage(job)"
			:progress="operationPercent(job)"
			:bytes-per-second="job.phase === 'downloading' ? job.bytes_per_second : undefined"
			:cancellable="job.cancellable"
			:cancelling="operations.cancelling.value === job.id"
			@cancel="operations.cancel(job.id)"
		/>
		<div v-if="loading" class="section-loading" role="status">
			<span class="spinner" />Загружаем библиотеку…
		</div>
		<template v-else-if="current">
			<a class="back-link" href="#/library"><AppIcon name="arrow" :size="16" />Все сборки</a>
			<div class="instance-hero">
				<img v-if="current.icon" :src="current.icon" width="90" height="90" alt="Иконка сборки" />
				<div v-else class="instance-icon"><AppIcon name="game" :size="38" /></div>
				<div class="instance-hero-copy">
					<span v-if="current.kind === 'official'" class="official-library-badge"
						><AppIcon name="check" :size="12" />Официальная NCreate</span
					>
					<span class="quiet-badge">{{ statusLabel(current.status) }}</span>
					<p>
						{{ modCountLabel(current.mod_count ?? mods.length) }} ·
						{{ (current.memory_mb / 1024).toFixed(1) }} ГБ RAM
					</p>
					<small>{{ lastPlayed(current) }}</small>
				</div>
				<button
					class="button primary"
					:disabled="
						busy || instanceBusy(current.id) || (current.status === 'ready' && !accountName)
					"
					@click="installOrPlay(current)"
				>
					<AppIcon name="game" />{{
						current.status === 'running'
							? 'Остановить'
							: current.status === 'ready'
								? 'Играть'
								: 'Установить Minecraft'
					}}
				</button>
			</div>
			<p v-if="!accountName" class="instance-account-note">
				Для запуска выбери <a href="#/accounts">аккаунт Minecraft</a>. Устанавливать файлы можно уже
				сейчас.
			</p>
			<p
				v-if="current.kind === 'official' && officialUpdateStatus[current.id]"
				class="official-update-inline"
				role="status"
			>
				{{
					officialUpdateStatus[current.id].phase === 'checking'
						? 'Проверяем обновления сборки…'
						: officialUpdateStatus[current.id].phase === 'available'
							? `Доступно обновление ${officialUpdateStatus[current.id].version}`
							: officialUpdateStatus[current.id].phase === 'current'
								? 'Сборка актуальна'
								: 'Не удалось проверить обновления сборки'
				}}
			</p>
			<div class="instance-toolbar">
				<button
					class="button subtle"
					:disabled="busy || instanceBusy(current.id) || current.status === 'running'"
					@click="showManage(current, 'configure')"
				>
					<AppIcon name="settings" :size="16" />Параметры
				</button>
				<button
					v-if="current.kind === 'official'"
					class="button secondary"
					:disabled="busy || instanceBusy(current.id)"
					@click="checkOfficialUpdate"
				>
					<AppIcon name="refresh" :size="16" />Обновления NCreate
				</button>
				<button
					class="button subtle"
					:disabled="busy || instanceBusy(current.id)"
					@click="showManage(current, 'rename')"
				>
					Переименовать</button
				><button
					class="button subtle"
					:disabled="busy || instanceBusy(current.id) || current.status === 'running'"
					@click="chooseIcon(current)"
				>
					Иконка</button
				><button class="button subtle" @click="operation(() => gameApi.openFolder(current!.id))">
					<AppIcon name="folder" :size="16" />Папка</button
				><button
					class="button subtle"
					:disabled="busy || instanceBusy(current.id)"
					@click="showManage(current, 'duplicate')"
				>
					Дублировать</button
				><button
					class="button subtle"
					:disabled="busy || instanceBusy(current.id)"
					@click="exportPack(current)"
				>
					Экспорт .mrpack</button
				><button
					class="button delete-button"
					:disabled="busy || instanceBusy(current.id) || current.status === 'running'"
					@click="showManage(current, 'delete')"
				>
					Удалить
				</button>
			</div>
			<div class="content-section-heading">
				<div>
					<h2>
						Моды сборки <span>{{ mods.length }}</span>
					</h2>
					<p>
						{{
							current.kind === 'official'
								? 'Официальные файлы обновляются отдельно. Свои моды можно добавлять и удалять.'
								: 'Включай, обновляй и управляй установленным контентом.'
						}}
					</p>
				</div>
				<div class="heading-actions">
					<a class="button secondary" :href="`#/content?instance=${current.id}`"
						><AppIcon name="plus" :size="17" />Добавить моды</a
					><button
						class="button secondary"
						:disabled="busy || instanceBusy(current.id)"
						@click="checkUpdates"
					>
						<AppIcon name="refresh" :size="17" />Проверить обновления
					</button>
				</div>
			</div>
			<div v-if="updates.length" class="update-summary">
				<AppIcon name="refresh" :size="20" /><span
					>Доступно совместимых обновлений: {{ updates.length }}</span
				><button
					class="button primary"
					:disabled="busy || instanceBusy(current.id)"
					@click="showUpdates(updates.map((update) => update.content_id))"
				>
					Обновить все
				</button>
			</div>
			<div v-if="!mods.length" class="empty-state compact-empty">
				<AppIcon name="game" :size="34" />
				<h2>Чистый мир возможностей</h2>
				<p v-if="current.kind === 'official'">
					Официальные файлы появятся здесь после установки. Свои моды можно добавить отдельно.
				</p>
				<p v-else>В этой сборке ещё нет модов.<br />Открой каталог и выбери совместимый контент.</p>
				<a class="button secondary" :href="`#/content?instance=${current.id}`"
					>Перейти к модам<AppIcon name="arrow" :size="16"
				/></a>
			</div>
			<template v-else
				><label class="sr-only" for="mod-filter">Найти установленный мод</label
				><input
					id="mod-filter"
					v-model="modFilter"
					class="mod-filter"
					name="mod-filter"
					placeholder="Найти мод в сборке…"
					autocomplete="off"
				/>
				<div class="installed-mod-list">
					<div
						v-for="mod in visibleMods"
						:key="mod.id"
						class="installed-mod"
						:class="{ disabled: !mod.enabled }"
					>
						<div class="mod-monogram" aria-hidden="true">
							{{ mod.name.charAt(0).toUpperCase() }}
						</div>
						<div class="mod-description">
							<a v-if="mod.project_id" :href="`#/content/${mod.project_id}`">{{ mod.name }}</a
							><strong v-else>{{ mod.name }}</strong>
							<p>
								{{
									mod.version_number ||
									(mod.version_id ? `Версия ${mod.version_id.slice(0, 8)}` : 'Локальный файл')
								}}
								· {{ mod.enabled ? 'Включён' : 'Выключен'
								}}<span v-if="updates.some((update) => update.content_id === mod.id)">
									· Есть обновление</span
								><span v-if="mod.managed"> · Управляется NCreate</span>
							</p>
						</div>
						<button
							v-if="updates.some((update) => update.content_id === mod.id)"
							class="button subtle"
							:disabled="
								mod.managed || busy || instanceBusy(current.id) || current.status === 'running'
							"
							@click="showUpdates([mod.id])"
						>
							Обновить</button
						><button
							class="switch"
							:class="{ on: mod.enabled }"
							role="switch"
							:aria-checked="mod.enabled"
							:aria-label="
								mod.managed
									? `${mod.name}: управляется официальной сборкой NCreate`
									: `${mod.enabled ? 'Выключить' : 'Включить'} ${mod.name}`
							"
							:title="mod.managed ? 'Управляется официальной сборкой NCreate' : undefined"
							:disabled="
								mod.managed || busy || instanceBusy(current.id) || current.status === 'running'
							"
							@click="toggleMod(mod)"
						>
							<span /></button
						><button
							class="icon-button"
							:aria-label="
								mod.managed
									? `${mod.name}: управляется официальной сборкой NCreate`
									: `Удалить ${mod.name}`
							"
							:title="mod.managed ? 'Управляется официальной сборкой NCreate' : undefined"
							:disabled="
								mod.managed || busy || instanceBusy(current.id) || current.status === 'running'
							"
							@click="showRemoveMod(mod)"
						>
							<AppIcon name="trash" :size="17" />
						</button>
					</div>
				</div>
				<p v-if="!visibleMods.length" class="list-empty" role="status">
					По этому запросу модов не найдено.
				</p></template
			>
			<div class="instance-footer">
				<span>{{ current.directory }}</span
				><button
					v-if="rollbackAvailable"
					class="button subtle"
					:disabled="busy || instanceBusy(current.id) || current.status === 'running'"
					@click="rollbackDialog?.showModal()"
				>
					Восстановить предыдущую версию
				</button>
			</div>
		</template>
		<div v-else-if="detail" class="empty-state">
			<AppIcon name="info" :size="36" />
			<h2>Сборка не найдена</h2>
			<p>Она могла быть удалена или перемещена.</p>
			<a href="#/library" class="button secondary">Открыть библиотеку</a>
		</div>
		<div v-else-if="!libraryInstances.length" class="empty-state library-empty">
			<div class="library-empty-art"><AppIcon name="folder" :size="48" /><span>+</span></div>
			<h2>Первый мир начинается с тебя</h2>
			<p>
				Создай свою сборку или установи NCreate Server на главной.<br />После установки она появится
				здесь вместе с твоими сборками.
			</p>
			<button class="button primary" @click="showCreate">
				<AppIcon name="plus" />Создать свою сборку</button
			><button class="button subtle" @click="importPack">Импортировать .mrpack</button>
		</div>
		<div v-else class="instance-grid">
			<article v-for="instance in libraryInstances" :key="instance.id" class="instance-card">
				<a :href="`#/library/${instance.id}`" class="instance-card-link"
					><div class="instance-cover">
						<img v-if="instance.icon" :src="instance.icon" width="80" height="80" alt="" /><AppIcon
							v-else
							name="game"
							:size="45"
						/><span class="loader-chip">{{ loaderNames[instance.loader] }}</span>
					</div>
					<span v-if="instance.kind === 'official'" class="official-library-badge"
						><AppIcon name="check" :size="12" />Официальная NCreate</span
					>
					<h2>{{ instance.name }}</h2>
					<p>
						Minecraft {{ instance.game_version }} · {{ modCountLabel(instance.mod_count ?? 0) }}
					</p>
					<small>{{ lastPlayed(instance) }}</small></a
				>
				<div class="instance-card-bottom">
					<span class="instance-state">{{
						instance.kind === 'official' && officialUpdateStatus[instance.id]?.phase === 'available'
							? 'Доступно обновление'
							: statusLabel(instance.status)
					}}</span
					><button
						class="button primary"
						:disabled="
							busy || instanceBusy(instance.id) || (instance.status === 'ready' && !accountName)
						"
						@click="installOrPlay(instance)"
					>
						<AppIcon name="game" :size="16" />{{
							instance.status === 'running'
								? 'Стоп'
								: instance.status === 'ready'
									? 'Играть'
									: 'Установить'
						}}
					</button>
					<details class="instance-menu">
						<summary aria-label="Действия со сборкой">⋯</summary>
						<div>
							<button
								:disabled="instanceBusy(instance.id) || instance.status === 'running'"
								@click="showManage(instance, 'configure')"
							>
								Параметры</button
							><a :href="`#/library/${instance.id}`">Открыть сборку</a
							><button
								:disabled="busy || instanceBusy(instance.id) || instance.status === 'running'"
								@click="showManage(instance, 'rename')"
							>
								Переименовать</button
							><button
								:disabled="busy || instanceBusy(instance.id) || instance.status === 'running'"
								@click="chooseIcon(instance)"
							>
								Изменить иконку</button
							><button @click="operation(() => gameApi.openFolder(instance.id))">
								Открыть папку</button
							><button
								:disabled="instanceBusy(instance.id) || instance.status === 'running'"
								@click="showManage(instance, 'duplicate')"
							>
								Дублировать</button
							><button :disabled="instanceBusy(instance.id)" @click="exportPack(instance)">
								Экспорт .mrpack</button
							><button
								class="danger-text"
								:disabled="instanceBusy(instance.id) || instance.status === 'running'"
								@click="showManage(instance, 'delete')"
							>
								Удалить
							</button>
						</div>
					</details>
				</div>
			</article>
		</div>
		<dialog
			ref="editionUpdateDialog"
			class="modal update-dialog"
			aria-labelledby="edition-update-title"
		>
			<button
				class="modal-close icon-button"
				aria-label="Закрыть обновление NCreate"
				:disabled="busy"
				@click="editionUpdateDialog?.close()"
			>
				<AppIcon name="close" />
			</button>
			<p class="eyebrow">ОФИЦИАЛЬНАЯ СБОРКА NCREATE</p>
			<h2 id="edition-update-title">Обновление сборки</h2>
			<template v-if="editionPlan"
				><p class="modal-intro">
					{{ editionPlan.from_version || 'Первая установка' }} → {{ editionPlan.to_version }} ·
					{{ (editionPlan.download_bytes / 1048576).toFixed(1) }} МБ
				</p>
				<p v-if="!editionHasChanges" class="field-help" role="status">
					Сборка актуальна. Изменений для установки нет.
				</p>
				<p v-else class="field-help">Доступно обновление. Проверьте изменения перед установкой.</p>
				<div class="update-plan-counts">
					<span>+ {{ editionPlan.added.length }} новых</span
					><span>↻ {{ editionPlan.changed.length }} изменённых</span
					><span>− {{ editionPlan.removed.length }} удалённых</span>
				</div>
				<p class="field-help">
					Лаунчер обновляет только файлы, которыми управляет сборка. Миры, скриншоты и личные
					настройки сохраняются.
				</p>
				<div v-if="editionPlan.conflicts.length" class="compatibility-warning">
					Локальные изменения требуют внимания:
					<ul>
						<li v-for="path in editionPlan.conflicts" :key="path">{{ path }}</li>
					</ul>
				</div>
				<details v-if="editionHasChanges" class="update-plan-files">
					<summary>Изменения файлов</summary>
					<ul>
						<li v-for="path in editionPlan.added" :key="'add' + path">+ {{ path }}</li>
						<li v-for="path in editionPlan.changed" :key="'change' + path">↻ {{ path }}</li>
						<li v-for="path in editionPlan.removed" :key="'remove' + path">− {{ path }}</li>
					</ul>
				</details>
				<p v-if="editionPlan.changelog" class="update-changelog">{{ editionPlan.changelog }}</p>
				<div v-if="error" class="banner error" role="alert">{{ error }}</div>
				<div class="dialog-actions">
					<button class="button subtle" :disabled="busy" @click="editionUpdateDialog?.close()">
						Отмена</button
					><button
						class="button primary"
						:disabled="busy || editionPlan.conflicts.length > 0 || !editionHasChanges"
						@click="applyOfficialUpdate"
					>
						Обновить сборку
					</button>
				</div></template
			>
		</dialog>
		<dialog
			ref="createDialog"
			class="modal create-instance-modal"
			aria-labelledby="create-instance-title"
		>
			<button
				class="modal-close icon-button"
				aria-label="Закрыть создание сборки"
				:disabled="busy"
				@click="createDialog?.close()"
			>
				<AppIcon name="close" />
			</button>
			<div class="modal-symbol"><AppIcon name="game" :size="27" /></div>
			<p class="eyebrow">НОВОЕ ПРИКЛЮЧЕНИЕ</p>
			<h2 id="create-instance-title">Создать сборку</h2>
			<p class="modal-intro">
				Выбери Minecraft и загрузчик.<br />Файлы установятся из официальных источников.
			</p>
			<form @submit.prevent="createInstance">
				<label for="instance-name">Название</label
				><input
					id="instance-name"
					ref="nameInput"
					v-model="createName"
					name="instance-name"
					autocomplete="off"
					maxlength="80"
					placeholder="Например, Мой новый мир"
					required
					:disabled="busy"
				/>
				<div class="form-columns">
					<div>
						<label for="instance-loader">Загрузчик</label
						><select id="instance-loader" v-model="loader" :disabled="busy || metadataLoading">
							<option v-for="(name, id) in loaderNames" :key="id" :value="id">{{ name }}</option>
						</select>
					</div>
					<div>
						<label for="instance-version">Minecraft</label
						><select
							id="instance-version"
							v-model="gameVersion"
							:disabled="busy || metadataLoading"
							required
						>
							<option v-if="!visibleVersions.length" value="">
								{{ metadataLoading ? 'Загружаем…' : 'Нет доступных версий' }}
							</option>
							<option v-for="version in visibleVersions" :key="version.id" :value="version.id">
								{{ version.id }}
							</option>
						</select>
					</div>
				</div>
				<label v-if="loader !== 'vanilla'" for="loader-version"
					>Версия {{ loaderNames[loader] }}</label
				><select
					v-if="loader !== 'vanilla'"
					id="loader-version"
					v-model="loaderVersion"
					:disabled="busy || metadataLoading"
					required
				>
					<option v-if="!loaderVersions.length" value="">
						{{ metadataLoading ? 'Загружаем…' : 'Нет совместимой версии' }}
					</option>
					<option v-for="version in loaderVersions" :key="version.id" :value="version.id">
						{{ version.id }}{{ version.stable ? '' : ' · preview' }}
					</option></select
				><label class="checkbox-row"
					><input v-model="includeSnapshots" type="checkbox" />Показывать snapshots Minecraft</label
				>
				<div v-if="metadataError" class="banner error" role="alert">
					{{ metadataError
					}}<button
						type="button"
						class="icon-button"
						aria-label="Повторить загрузку версий"
						@click="loadMetadata"
					>
						<AppIcon name="refresh" :size="16" />
					</button>
				</div>
				<p class="field-help">
					RAM: {{ (settings.memory_mb / 1024).toFixed(1) }} ГБ · Свой профиль, отдельно от
					официальных сборок NCreate.
				</p>
				<button class="button primary full-width" :disabled="busy || metadataLoading">
					{{ busy ? 'Создаём…' : 'Создать сборку' }}<AppIcon name="plus" :size="18" />
				</button>
			</form>
		</dialog>
		<dialog ref="manageDialog" class="modal" aria-labelledby="manage-title">
			<button
				class="modal-close icon-button"
				aria-label="Закрыть окно"
				:disabled="busy"
				@click="manageDialog?.close()"
			>
				<AppIcon name="close" />
			</button>
			<h2 id="manage-title">
				{{
					manageAction === 'delete'
						? 'Удалить сборку?'
						: manageAction === 'duplicate'
							? 'Создать копию'
							: manageAction === 'configure'
								? 'Параметры сборки'
								: 'Название сборки'
				}}
			</h2>
			<p class="modal-intro">
				{{
					manageAction === 'delete'
						? `«${managed?.name}» исчезнет из библиотеки. Файлы сборки будут перемещены в локальную корзину лаунчера.`
						: manageAction === 'configure'
							? 'RAM и Java используются только этой сборкой. Версия Minecraft и загрузчик сохраняются.'
							: 'Имя помогает найти свой мир в библиотеке.'
				}}
			</p>
			<form @submit.prevent="manageInstance">
				<template v-if="manageAction === 'configure'"
					><label for="instance-memory"
						>Оперативная память · {{ (manageMemory / 1024).toFixed(1) }} ГБ</label
					><input
						id="instance-memory"
						v-model.number="manageMemory"
						type="range"
						min="1024"
						max="32768"
						step="512"
						:disabled="busy"
					/><label for="instance-java" class="field-label-spaced"
						>Путь к исполняемому файлу Java</label
					><input
						id="instance-java"
						v-model="manageJava"
						name="instance-java"
						autocomplete="off"
						spellcheck="false"
						placeholder="Автоматический выбор установленной Java"
						:disabled="busy"
					/>
					<p class="field-help">
						Укажи путь к java, например /usr/lib/jvm/java-21-openjdk/bin/java. Лаунчер проверит
						версию перед установкой и запуском.
					</p></template
				>
				<template v-else-if="manageAction !== 'delete'"
					><label for="manage-name">Название</label
					><input
						id="manage-name"
						v-model="manageName"
						name="manage-name"
						maxlength="80"
						autocomplete="off"
						required
				/></template>
				<div v-if="error" class="banner error" role="alert">{{ error }}</div>
				<div class="dialog-actions">
					<button
						type="button"
						class="button subtle"
						:disabled="busy"
						@click="manageDialog?.close()"
					>
						Отмена</button
					><button
						class="button"
						:class="manageAction === 'delete' ? 'danger' : 'primary'"
						:disabled="busy"
					>
						{{ busy ? 'Сохраняем…' : manageAction === 'delete' ? 'Удалить сборку' : 'Сохранить' }}
					</button>
				</div>
			</form>
		</dialog>
		<dialog ref="updateDialog" class="modal update-dialog" aria-labelledby="update-title">
			<button
				class="modal-close icon-button"
				aria-label="Закрыть обновление модов"
				:disabled="busy"
				@click="updateDialog?.close()"
			>
				<AppIcon name="close" />
			</button>
			<p class="eyebrow">СОВМЕСТИМЫЕ ОБНОВЛЕНИЯ</p>
			<h2 id="update-title">Обновить моды</h2>
			<p class="modal-intro">
				Подбираем версии для Minecraft {{ current?.game_version }} и
				{{ current ? loaderNames[current.loader] : '' }}. Предыдущее состояние можно восстановить.
			</p>
			<ul class="update-list">
				<li
					v-for="update in updates.filter((item) => selectedUpdateIds.includes(item.content_id))"
					:key="update.content_id"
				>
					<strong>{{ update.name }}</strong
					><span>{{ update.current_version }} → {{ update.next_version }}</span>
				</li>
			</ul>
			<div v-if="error" class="banner error" role="alert">{{ error }}</div>
			<div class="dialog-actions">
				<button class="button subtle" :disabled="busy" @click="updateDialog?.close()">Отмена</button
				><button
					class="button primary"
					:disabled="busy || !selectedUpdateIds.length"
					@click="applyUpdates"
				>
					Обновить {{ selectedUpdateIds.length }}
				</button>
			</div>
		</dialog>
		<dialog ref="removeModDialog" class="modal" aria-labelledby="remove-mod-title">
			<h2 id="remove-mod-title">Удалить мод?</h2>
			<p class="modal-intro">
				{{ removingMod?.name }} будет удалён из этой сборки. Чтобы временно отключить мод, используй
				переключатель в списке.
			</p>
			<div v-if="error" class="banner error" role="alert">{{ error }}</div>
			<div class="dialog-actions">
				<button class="button subtle" :disabled="busy" @click="removeModDialog?.close()">
					Отмена</button
				><button class="button danger" :disabled="busy" @click="removeMod">Удалить мод</button>
			</div>
		</dialog>
		<dialog ref="rollbackDialog" class="modal" aria-labelledby="rollback-title">
			<h2 id="rollback-title">Восстановить состояние?</h2>
			<p class="modal-intro">
				Вернём управляемые файлы к сохранённому состоянию до последнего обновления. Миры и личные
				файлы не заменяются.
			</p>
			<div v-if="error" class="banner error" role="alert">{{ error }}</div>
			<div class="dialog-actions">
				<button class="button subtle" :disabled="busy" @click="rollbackDialog?.close()">
					Отмена</button
				><button class="button primary" :disabled="busy" @click="rollback">Восстановить</button>
			</div>
		</dialog>
	</section>
</template>
