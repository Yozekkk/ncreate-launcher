<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { operationTitle, operationMessage } from './operation-labels'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'
import AppIcon from './components/AppIcon.vue'
import ElyLoginDialog from './components/ElyLoginDialog.vue'
import LibraryPage from './components/LibraryPage.vue'
import ContentPage from './components/ContentPage.vue'
import OperationPanel from './components/OperationPanel.vue'
import { isTerminalPhase, operationPercent, useGameOperations } from './game-operations'
import { parseLauncherRoute, type Page } from './routes'
import {
	officialEdition,
	type Account,
	type EditionId,
	type Settings,
	type Skin,
	type Snapshot,
} from './models'
import {
	gameApi,
	loaderNames,
	type EditionAvailability,
	type Instance,
	type UpdatePlan,
} from './game-api'
import { configureEditionInstaller, canInstallEdition, installEdition } from './edition-installer'
import { formatUpdateBytes, useLauncherUpdates } from './launcher-updates'
import { renderProjectMarkdown } from './project-markdown'
import { messages } from './i18n'

const navigation: { id: Page; label: string; icon: string }[] = [
	{ id: 'home', label: messages.navigation.home, icon: 'home' },
	{ id: 'library', label: 'Библиотека', icon: 'folder' },
	{ id: 'content', label: 'Моды и сборки', icon: 'game' },
	{ id: 'accounts', label: messages.navigation.accounts, icon: 'accounts' },
	{ id: 'settings', label: messages.navigation.settings, icon: 'settings' },
]
const page = ref<Page>('home')
const operations = useGameOperations()
const launcherUpdates = useLauncherUpdates()
const launcherUpdateNotes = computed(() =>
	renderProjectMarkdown(launcherUpdates.candidate.value?.notes || ''),
)
const routeDetail = ref<string | null>(null)
const snapshot = ref<Snapshot | null>(null)
const draft = ref<Settings | null>(null)
const loading = ref(true)
const busy = ref(false)
const editionAvailability = ref<EditionAvailability | null>(null)
const editionLoading = ref(true)
const currentEdition = computed(() =>
	editionAvailability.value?.channel === (snapshot.value?.settings.release_channel || 'stable')
		? editionAvailability.value
		: null,
)
const editionModel = computed(() => ({
	...officialEdition,
	manifest:
		currentEdition.value?.available && currentEdition.value.manifest
			? JSON.stringify(currentEdition.value.manifest)
			: null,
}))
const officialManifest = computed(() =>
	currentEdition.value?.available ? currentEdition.value.manifest : null,
)
const officialInstance = ref<Instance | null>(null)
const officialLoading = ref(true)
const officialLoadError = ref(false)
const officialUpdate = ref<UpdatePlan | null>(null)
const officialUpdatePhase = ref<'checking' | 'current' | 'available' | 'error'>('checking')
const officialBusy = ref(false)
const officialJob = computed(() =>
	operations.active.value.find(
		(job) =>
			(job.operation === 'install_edition' && !job.instance_id) ||
			(['update_edition', 'install_game'].includes(job.operation) &&
				job.instance_id === officialInstance.value?.id),
	),
)
const officialDownloadBytes = computed(
	() => officialManifest.value?.files.reduce((sum, file) => sum + file.size, 0) ?? null,
)
const bytes = new Intl.NumberFormat('ru', { maximumFractionDigits: 1 })
const officialSize = computed(() =>
	officialDownloadBytes.value === null
		? 'Размер уточняется'
		: `Файлы сборки: ${bytes.format(officialDownloadBytes.value / 1048576)} МБ`,
)
let editionGeneration = 0
async function loadEditions() {
	const generation = ++editionGeneration
	editionLoading.value = true
	try {
		const result = await gameApi.editions(snapshot.value?.settings.release_channel || 'stable')
		if (mounted && generation === editionGeneration)
			editionAvailability.value = result.find((item) => item.id === officialEdition.id) ?? null
	} catch {
		if (mounted && generation === editionGeneration) editionAvailability.value = null
	} finally {
		if (mounted && generation === editionGeneration) editionLoading.value = false
	}
}
let officialGeneration = 0
async function loadOfficialState() {
	const generation = ++officialGeneration
	officialLoading.value = true
	try {
		const instances = await gameApi.instances()
		if (!mounted || generation !== officialGeneration) return
		officialLoadError.value = false
		const instance = instances.find(
			(item) => item.kind === 'official' && item.edition === officialEdition.id,
		)
		officialInstance.value = instance ?? null
		officialUpdate.value = null
		if (!instance) {
			officialUpdatePhase.value = 'current'
			return
		}
		officialUpdatePhase.value = 'checking'
		try {
			const plan = await gameApi.checkEditionUpdate(
				instance.id,
				snapshot.value?.settings.release_channel || 'stable',
			)
			if (mounted && generation === officialGeneration) {
				officialUpdate.value = plan
				officialUpdatePhase.value =
					plan.from_version !== plan.to_version ||
					plan.added.length + plan.changed.length + plan.removed.length > 0
						? 'available'
						: 'current'
			}
		} catch {
			if (mounted && generation === officialGeneration) officialUpdatePhase.value = 'error'
		}
	} catch {
		if (mounted && generation === officialGeneration) {
			officialLoadError.value = true
			officialUpdatePhase.value = 'error'
		}
	} finally {
		if (mounted && generation === officialGeneration) officialLoading.value = false
	}
}
configureEditionInstaller(async ({ editionId }) => {
	await operations.track(
		await gameApi.installEdition(editionId, snapshot.value?.settings.release_channel || 'stable'),
	)
	notice.value = 'Установка NCreate началась'
	return true
})
watch(
	() => snapshot.value?.settings.release_channel,
	() => {
		void loadEditions()
		void loadOfficialState()
	},
)
watch(
	() =>
		operations.jobs.value
			.filter(
				(job) =>
					isTerminalPhase(job.phase) &&
					['install_edition', 'update_edition', 'install_game', 'launch', 'stop'].includes(
						job.operation,
					),
			)
			.map((job) => job.id)
			.join(','),
	() => void loadOfficialState(),
)
const installingEdition = ref<EditionId | null>(null)
const officialActionBusy = computed(
	() => installingEdition.value !== null || officialBusy.value || !!officialJob.value,
)
const error = ref('')
const notice = ref('')
let noticeTimer: ReturnType<typeof setTimeout> | undefined
watch(notice, (value) => {
	clearTimeout(noticeTimer)
	if (value)
		noticeTimer = setTimeout(() => {
			notice.value = ''
		}, 5000)
})
const online = ref(navigator.onLine)
const authenticating = ref(false)
const authProvider = ref<'microsoft' | 'ely_by'>('microsoft')
const elyLoginDialog = ref<InstanceType<typeof ElyLoginDialog> | null>(null)
const authStage = ref<'browser' | 'minecraft'>('browser')
const eventCleanup: UnlistenFn[] = []
let mounted = true
const autostart = ref(false)
const autostartAvailable = ref(false)
const version = ref('…')
const license = ref('')
const fontLicenses = ref('')
const skins = reactive<Record<string, Skin | undefined>>({})
const skinLoading = reactive<Record<string, boolean>>({})
const generations = new Map<string, number>()
const offlineDialog = ref<HTMLDialogElement | null>(null)
const accountDialog = ref<HTMLDialogElement | null>(null)
const aboutDialog = ref<HTMLDialogElement | null>(null)
const nicknameInput = ref<HTMLInputElement | null>(null)
const nickname = ref('')
const nicknameError = ref('')
const selectedUuid = ref('')
const editNickname = ref('')
const deleting = ref(false)
const activeAccount = computed(() => snapshot.value?.accounts.find((account) => account.active))
const selectedAccount = computed(() =>
	snapshot.value?.accounts.find((account) => account.uuid === selectedUuid.value),
)
const settingsChanged = computed(
	() => JSON.stringify(draft.value) !== JSON.stringify(snapshot.value?.settings),
)
const usernameValid = (value: string) => /^[A-Za-z0-9_]{3,16}$/.test(value)
const accountType = (account: Account) =>
	account.kind === 'microsoft'
		? 'Microsoft'
		: account.kind === 'ely_by'
			? 'Ely.by'
			: skins[account.uuid]?.provider === 'ely_by'
				? 'Offline / Ely.by skin'
				: 'Offline'
const errorMessage = (reason: unknown) =>
	typeof reason === 'string'
		? reason
		: reason instanceof Error
			? reason.message
			: 'Не удалось выполнить действие. Попробуйте ещё раз.'

function showElyLogin() {
	authProvider.value = 'ely_by'
	void elyLoginDialog.value?.open()
}
function updateRoute() {
	const route = parseLauncherRoute(location.hash)
	page.value = route.page
	routeDetail.value = route.detail
}
function focusMain() {
	document.getElementById('main-content')?.focus()
}
function setPage(value: Page) {
	location.hash = `/${value}`
	page.value = value
	error.value = ''
}
function connectivity() {
	online.value = navigator.onLine
	if (online.value)
		for (const account of snapshot.value?.accounts || [])
			if (skins[account.uuid]?.status === 'network_error') void loadSkin(account, true)
}
async function startEditionInstall() {
	if (
		officialActionBusy.value ||
		officialInstance.value ||
		officialLoadError.value ||
		!canInstallEdition(editionModel.value)
	)
		return
	installingEdition.value = officialEdition.id
	error.value = ''
	try {
		if (!(await installEdition(editionModel.value))) notice.value = 'Установка отменена'
	} catch (reason) {
		error.value = errorMessage(reason)
	} finally {
		installingEdition.value = null
	}
}
async function resumeOfficialInstall() {
	const instance = officialInstance.value
	if (!instance || officialActionBusy.value) return
	officialBusy.value = true
	error.value = ''
	try {
		await operations.track(await gameApi.installGame(instance.id))
		notice.value = 'Продолжаем установку Minecraft'
	} catch (reason) {
		error.value = errorMessage(reason)
	} finally {
		officialBusy.value = false
	}
}
async function updateOfficial() {
	const instance = officialInstance.value
	if (!instance || officialActionBusy.value) return
	officialBusy.value = true
	error.value = ''
	try {
		const plan = await gameApi.checkEditionUpdate(
			instance.id,
			snapshot.value?.settings.release_channel || 'stable',
		)
		if (plan.conflicts.length) {
			error.value =
				'Обновление затрагивает ваши файлы. Откройте сборку в библиотеке, чтобы проверить конфликт.'
			return
		}
		if (
			plan.from_version === plan.to_version &&
			!plan.added.length &&
			!plan.changed.length &&
			!plan.removed.length
		) {
			notice.value = 'Сборка уже актуальна'
			return
		}
		await operations.track(
			await gameApi.applyEditionUpdate(
				instance.id,
				snapshot.value?.settings.release_channel || 'stable',
			),
		)
		notice.value = 'Обновление сборки началось'
	} catch (reason) {
		error.value = errorMessage(reason)
	} finally {
		officialBusy.value = false
	}
}
async function playOfficial() {
	const instance = officialInstance.value
	if (!instance || instance.status !== 'ready' || officialActionBusy.value) return
	if (!activeAccount.value) {
		error.value = 'Добавьте или выберите активный аккаунт на странице «Аккаунты».'
		return
	}
	officialBusy.value = true
	error.value = ''
	try {
		await gameApi.launch(instance.id)
		notice.value = 'Minecraft запущен'
		await loadOfficialState()
	} catch (reason) {
		error.value = errorMessage(reason)
	} finally {
		officialBusy.value = false
	}
}
async function loadSkin(account: Account, force = false) {
	if (!force && (skins[account.uuid] || skinLoading[account.uuid])) return
	const generation = (generations.get(account.uuid) || 0) + 1
	generations.set(account.uuid, generation)
	skinLoading[account.uuid] = true
	try {
		const result = await invoke<Skin>('skin', { uuid: account.uuid, force })
		if (
			generations.get(account.uuid) === generation &&
			snapshot.value?.accounts.some((item) => item.uuid === account.uuid)
		)
			skins[account.uuid] = result
	} catch {
		if (generations.get(account.uuid) === generation)
			skins[account.uuid] = {
				provider: 'fallback',
				head: null,
				texture: null,
				status: 'network_error',
			}
	} finally {
		if (generations.get(account.uuid) === generation) skinLoading[account.uuid] = false
	}
}
function applySnapshot(result: Snapshot) {
	const pendingSettings = draft.value && settingsChanged.value ? draft.value : null
	snapshot.value = result
	if (!pendingSettings || JSON.stringify(pendingSettings) === JSON.stringify(result.settings))
		draft.value = {
			...result.settings,
			release_channel: result.settings.release_channel || 'stable',
		}
	for (const uuid of Object.keys(skins))
		if (!result.accounts.some((account) => account.uuid === uuid)) {
			delete skins[uuid]
			generations.set(uuid, (generations.get(uuid) || 0) + 1)
		}
	for (const account of result.accounts) void loadSkin(account)
}
async function initialize() {
	loading.value = true
	error.value = ''
	try {
		const result = await invoke<Snapshot>('snapshot')
		applySnapshot(result)
		if (result.settings.auto_updates)
			void launcherUpdates.check(result.settings.release_channel || 'stable')
	} catch (reason) {
		error.value = errorMessage(reason)
	} finally {
		loading.value = false
	}
}
async function mutate(command: string, args?: Record<string, unknown>, success?: string) {
	busy.value = true
	error.value = ''
	try {
		applySnapshot(await invoke<Snapshot>(command, args))
		if (success) notice.value = success
		return true
	} catch (reason) {
		error.value = errorMessage(reason)
		return false
	} finally {
		busy.value = false
	}
}
async function showOffline() {
	error.value = ''
	nickname.value = ''
	nicknameError.value = ''
	offlineDialog.value?.showModal()
	await nextTick()
	nicknameInput.value?.focus()
}
async function addOffline() {
	if (!usernameValid(nickname.value)) {
		nicknameError.value = 'От 3 до 16 символов: латинские буквы, цифры и _.'
		nicknameInput.value?.focus()
		return
	}
	if (await mutate('add_offline', { nickname: nickname.value }, 'Офлайн аккаунт добавлен'))
		offlineDialog.value?.close()
}
function showAccount(account: Account) {
	selectedUuid.value = account.uuid
	editNickname.value = account.nickname
	deleting.value = false
	error.value = ''
	accountDialog.value?.showModal()
}
async function renameAccount() {
	if (!selectedAccount.value || !usernameValid(editNickname.value)) {
		error.value = 'Никнейм: от 3 до 16 латинских букв, цифр и _.'
		return
	}
	const result = await mutate(
		'rename_offline',
		{ uuid: selectedUuid.value, nickname: editNickname.value },
		'Никнейм обновлён',
	)
	if (result) accountDialog.value?.close()
}
async function deleteAccount() {
	if (await mutate('remove_account', { uuid: selectedUuid.value }, 'Аккаунт удалён'))
		accountDialog.value?.close()
}
async function refreshAccount() {
	const uuid = selectedUuid.value
	if (await mutate('refresh_account', { uuid }, 'Данные профиля обновлены')) {
		const account = snapshot.value?.accounts.find((item) => item.uuid === uuid)
		if (account) void loadSkin(account, true)
	}
}
async function microsoftLogin() {
	authenticating.value = true
	authProvider.value = 'microsoft'
	authStage.value = 'browser'
	error.value = ''
	notice.value = ''
	try {
		const result = await invoke<Snapshot | null>('microsoft_login')
		if (result) {
			applySnapshot(result)
			notice.value = 'Microsoft аккаунт добавлен'
		}
	} catch (reason) {
		error.value = errorMessage(reason)
	} finally {
		authenticating.value = false
	}
}
async function cancelLogin() {
	try {
		await invoke('cancel_login')
	} catch (reason) {
		error.value = errorMessage(reason)
	}
}
async function toggleAutostart() {
	try {
		if (autostart.value) await disable()
		else await enable()
		autostart.value = await isEnabled()
	} catch (reason) {
		error.value = errorMessage(reason)
	}
}
async function saveSettings() {
	if (!draft.value) return
	const previous = snapshot.value?.settings
	if (await mutate('save_settings', { settings: { ...draft.value } }, 'Настройки сохранены')) {
		const settings = snapshot.value?.settings
		if (
			settings?.auto_updates &&
			(!previous?.auto_updates || previous.release_channel !== settings.release_channel)
		)
			void launcherUpdates.check(settings.release_channel)
	}
}
async function restart() {
	try {
		await invoke('restart_launcher')
	} catch (reason) {
		error.value = errorMessage(reason)
	}
}

onMounted(() => {
	updateRoute()
	void invoke<string | null>('initial_route')
		.then((route) => {
			if (mounted && route) {
				const parsed = parseLauncherRoute(route)
				location.hash = `/${parsed.page}${parsed.detail ? `/${parsed.detail}` : ''}`
				updateRoute()
			}
		})
		.catch(() => {})
	window.addEventListener('hashchange', updateRoute)
	window.addEventListener('online', connectivity)
	window.addEventListener('offline', connectivity)
	void listen<string>('ncreate-route', (event) => {
		const parsed = parseLauncherRoute(event.payload)
		location.hash = `/${parsed.page}${parsed.detail ? `/${parsed.detail}` : ''}`
		updateRoute()
	})
		.then((cleanup) => {
			if (mounted) eventCleanup.push(cleanup)
			else cleanup()
		})
		.catch(() => {})
	void listen<string>('auth-progress', (event) => {
		if (event.payload === 'browser' || event.payload === 'minecraft')
			authStage.value = event.payload
	})
		.then((cleanup) => {
			if (mounted) eventCleanup.push(cleanup)
			else cleanup()
		})
		.catch(() => {})
	void initialize()
	void fetch('/LICENSE.txt')
		.then((response) => response.text())
		.then((text) => {
			license.value = text
		})
		.catch(() => {
			license.value =
				'Не удалось загрузить текст. Лицензия находится в файле LICENSE исходного репозитория.'
		})
	void Promise.all(
		['/fonts/Manrope-OFL.txt', '/fonts/Unbounded-OFL.txt'].map((path) =>
			fetch(path).then((response) => response.text()),
		),
	)
		.then((texts) => {
			fontLicenses.value = texts.join('\n\n')
		})
		.catch(() => {})
	void getVersion()
		.then((value) => {
			version.value = value
		})
		.catch(() => {})
	void isEnabled()
		.then((value) => {
			autostart.value = value
			autostartAvailable.value = true
		})
		.catch(() => {})
})
onUnmounted(() => {
	clearTimeout(noticeTimer)
	mounted = false
	for (const cleanup of eventCleanup) cleanup()
	window.removeEventListener('hashchange', updateRoute)
	window.removeEventListener('online', connectivity)
	window.removeEventListener('offline', connectivity)
})
</script>

<template>
	<div
		class="launcher"
		:class="{
			oled: snapshot?.settings.theme === 'oled',
			'no-motion': snapshot?.settings.reduced_motion || snapshot?.settings.animations === false,
			'no-blur': snapshot?.settings.blur === false,
		}"
	>
		<a href="#main-content" class="skip-link" @click.prevent="focusMain">Перейти к содержимому</a>
		<aside class="sidebar" aria-label="Основная навигация">
			<a href="#/home" class="brand" @click="setPage('home')"
				><img src="/brand/logo.webp" width="46" height="46" alt="Логотип NCreate" /><span
					>NCreate<small>LAUNCHER</small></span
				></a
			>
			<div class="nav-label">ТВОЁ ПРИКЛЮЧЕНИЕ</div>
			<nav>
				<a
					v-for="item in navigation"
					:key="item.id"
					:href="`#/${item.id}`"
					:class="{ active: page === item.id }"
					:aria-current="page === item.id ? 'page' : undefined"
					@click="setPage(item.id)"
					><AppIcon :name="item.icon" /><span>{{ item.label }}</span
					><i v-if="page === item.id"
				/></a>
			</nav>
			<div class="sidebar-bottom">
				<div class="stage-note">
					<span class="status-dot" /> ТВОЙ МИР NCREATE
					<p>Большое приключение<br />начинается здесь.</p>
				</div>
				<button v-if="activeAccount" class="active-profile" @click="showAccount(activeAccount)">
					<img
						v-if="skins[activeAccount.uuid]?.head"
						:src="skins[activeAccount.uuid]?.head || ''"
						width="38"
						height="38"
						alt=""
					/><span v-else class="pixel-avatar" aria-hidden="true" /><span class="profile-name"
						>{{ activeAccount.nickname }}<small>Активный аккаунт</small></span
					><AppIcon name="arrow" :size="16" />
				</button>
				<button v-else class="active-profile" @click="setPage('accounts')">
					<span class="guest-avatar"><AppIcon name="accounts" /></span
					><span class="profile-name">Добавить аккаунт<small>Всё готово к началу</small></span
					><AppIcon name="plus" :size="16" />
				</button>
				<button class="about-link" @click="aboutDialog?.showModal()">
					NCreate Launcher <span>v{{ version }}</span>
				</button>
			</div>
		</aside>
		<main id="main-content" tabindex="-1" class="main-area">
			<header class="topbar">
				<span class="breadcrumb"
					>NCreate Launcher <span>/</span>
					{{ navigation.find((item) => item.id === page)?.label }}</span
				><span class="connection" :class="{ disconnected: !online }"
					><AppIcon name="wifi" :size="15" />{{ online ? 'В сети' : 'Нет подключения' }}</span
				>
			</header>
			<div v-if="!online" class="banner network" role="status">
				Нет подключения к интернету. Офлайн аккаунты и настройки доступны; скины загрузятся после
				подключения.
			</div>
			<div
				v-if="launcherUpdates.phase.value === 'available' && page !== 'settings'"
				class="banner launcher-update-banner"
				role="status"
			>
				<span
					>Доступно обновление NCreate Launcher {{ launcherUpdates.candidate.value?.version }}</span
				>
				<div class="launcher-update-banner-actions">
					<button class="button secondary" @click="setPage('settings')">Подробнее</button>
					<button class="icon-button" aria-label="Напомнить позже" @click="launcherUpdates.later()">
						<AppIcon name="close" :size="16" />
					</button>
				</div>
			</div>
			<div
				v-if="error && !offlineDialog?.open && !accountDialog?.open"
				class="banner error"
				role="alert"
			>
				<span>{{ error }}</span
				><button class="icon-button" aria-label="Закрыть сообщение" @click="error = ''">
					<AppIcon name="close" :size="16" />
				</button>
			</div>
			<div
				v-if="operations.error.value && page !== 'library' && page !== 'content'"
				class="banner error"
				role="alert"
			>
				<span>{{ operations.error.value }}</span
				><button
					class="icon-button"
					aria-label="Закрыть ошибку установки"
					@click="operations.clearError()"
				>
					<AppIcon name="close" :size="16" />
				</button>
			</div>
			<div v-if="notice" class="toast" role="status">
				<AppIcon name="check" :size="17" />{{ notice
				}}<button class="icon-button" aria-label="Закрыть уведомление" @click="notice = ''">
					<AppIcon name="close" :size="15" />
				</button>
			</div>
			<div v-if="loading" class="loading-screen" role="status">
				<img src="/brand/logo.webp" width="72" height="72" alt="" /><span class="spinner" />
				<h1>Готовим NCreate…</h1>
				<p>Загружаем аккаунты и настройки</p>
			</div>
			<div v-else-if="!snapshot" class="empty-state">
				<AppIcon name="info" :size="44" />
				<h1>Не удалось открыть лаунчер</h1>
				<p>Проверьте сообщение об ошибке и попробуйте снова.</p>
				<button class="button primary" @click="initialize">
					<AppIcon name="refresh" />Повторить
				</button>
			</div>
			<template v-else>
				<OperationPanel
					v-for="job in page === 'library' || page === 'content' ? [] : operations.active.value"
					:key="job.id"
					class="global-operation"
					:title="operationTitle(job.operation)"
					:message="operationMessage(job)"
					:progress="operationPercent(job)"
					:bytes-per-second="job.phase === 'downloading' ? job.bytes_per_second : undefined"
					:cancellable="job.cancellable"
					:cancelling="operations.cancelling.value === job.id"
					@cancel="operations.cancel(job.id)"
				/>
				<section v-if="page === 'home'" class="page home-page">
					<div class="hero-heading">
						<div>
							<p class="eyebrow"><span /> ТВОЙ МИР. ТВОИ ПРАВИЛА.</p>
							<h1>Твой путь в мир<br /><span>NCreate</span></h1>
							<p class="page-intro">
								Установи официальную сборку и присоединяйся к серверу.<br />Все файлы и обновления
								лаунчер подготовит сам.
							</p>
						</div>
						<div class="hero-mark" aria-hidden="true">
							N<span>C</span><small>CREATE YOUR WORLD</small>
						</div>
					</div>
					<article class="official-pack-card" aria-labelledby="official-pack-title">
						<div class="official-pack-top">
							<span>NCREATE / SERVER</span>
							<span class="official-pack-badge"
								><AppIcon name="check" :size="13" />Официальная NCreate</span
							>
						</div>
						<div class="official-pack-body">
							<div class="official-pack-art" aria-hidden="true">
								<span class="official-pack-halo" />
								<img src="/brand/logo.webp" width="512" height="512" alt="" />
								<span class="official-pack-art-word">NCREATE</span>
							</div>
							<div class="official-pack-copy">
								<p class="edition-label">{{ officialEdition.label }}</p>
								<h2 id="official-pack-title">{{ officialEdition.name }}</h2>
								<p class="official-pack-description">{{ officialEdition.description }}</p>
								<dl class="official-pack-facts">
									<div>
										<dt>Версия сборки</dt>
										<dd>
											{{ officialManifest?.version || officialInstance?.manifest_version || '—' }}
										</dd>
									</div>
									<div>
										<dt>Minecraft</dt>
										<dd>
											{{ officialManifest?.minecraft || officialInstance?.game_version || '—' }}
										</dd>
									</div>
									<div>
										<dt>Загрузчик</dt>
										<dd>
											{{
												loaderNames[
													officialManifest?.loader.kind || officialInstance?.loader || 'vanilla'
												]
											}}
											{{
												officialManifest?.loader.version || officialInstance?.loader_version || ''
											}}
										</dd>
									</div>
								</dl>
								<p class="official-pack-size">{{ officialSize }}</p>
								<p class="official-pack-status" role="status">
									<span
										class="official-pack-status-dot"
										:class="{ active: officialInstance?.status === 'ready' }"
									/>
									{{
										officialLoading || editionLoading
											? 'Проверяем сборку…'
											: officialLoadError
												? 'Не удалось проверить установку'
												: officialJob
													? operationTitle(officialJob.operation)
													: officialInstance?.status === 'running'
														? 'Игра запущена'
														: officialInstance &&
															  !['ready', 'running'].includes(officialInstance.status)
															? 'Нужно завершить установку Minecraft'
															: officialInstance && officialUpdatePhase === 'available'
																? `Доступно обновление ${officialUpdate?.to_version}`
																: officialInstance && officialUpdatePhase === 'error'
																	? 'Не удалось проверить обновления'
																	: officialInstance
																		? 'Готова к игре'
																		: canInstallEdition(editionModel)
																			? 'Готова к установке'
																			: 'Публикация сборки готовится'
									}}
								</p>
								<div v-if="officialJob" class="official-pack-progress" aria-live="polite">
									<div class="official-pack-progress-track">
										<span :style="{ width: `${operationPercent(officialJob) ?? 0}%` }" />
									</div>
									<small>{{ operationMessage(officialJob) }}</small>
								</div>
								<div class="official-pack-actions">
									<button
										v-if="
											officialInstance && !['ready', 'running'].includes(officialInstance.status)
										"
										class="button primary"
										:disabled="officialActionBusy"
										@click="resumeOfficialInstall"
									>
										<AppIcon name="refresh" :size="17" />Завершить установку
									</button>
									<button
										v-else-if="officialInstance && officialUpdatePhase === 'available'"
										class="button primary"
										:disabled="officialActionBusy || officialInstance.status === 'running'"
										@click="updateOfficial"
									>
										<AppIcon name="refresh" :size="17" />Обновить
									</button>
									<button
										v-else-if="officialInstance"
										class="button primary"
										:disabled="
											officialActionBusy || officialInstance.status !== 'ready' || !activeAccount
										"
										@click="playOfficial"
									>
										<AppIcon name="game" :size="17" />{{
											officialInstance.status === 'running' ? 'Игра запущена' : 'Играть'
										}}
									</button>
									<button
										v-else
										class="button primary"
										:disabled="
											officialLoading ||
											officialLoadError ||
											officialActionBusy ||
											!canInstallEdition(editionModel)
										"
										@click="startEditionInstall"
									>
										<AppIcon name="game" :size="17" />{{
											officialActionBusy
												? 'Устанавливаем…'
												: canInstallEdition(editionModel)
													? 'Установить'
													: 'Скоро'
										}}
									</button>
									<button
										v-if="
											officialInstance &&
											officialUpdatePhase === 'available' &&
											officialInstance.status === 'ready'
										"
										class="button secondary"
										:disabled="officialActionBusy || !activeAccount"
										@click="playOfficial"
									>
										Играть
									</button>
									<a
										v-if="officialInstance"
										class="button subtle"
										:href="`#/library/${officialInstance.id}`"
										>Открыть в библиотеке</a
									>
								</div>
								<p
									v-if="officialInstance?.status === 'ready' && !activeAccount"
									class="official-pack-hint"
								>
									Чтобы играть, <a href="#/accounts">выберите аккаунт Minecraft</a>.
								</p>
							</div>
						</div>
					</article>
					<div class="home-footer">
						<AppIcon name="shield" :size="20" />
						<p>
							Твой мир. Твои правила.<span
								>Сохраняй собственные миры и моды — обновляются только официальные файлы
								сборки.</span
							>
						</p>
						<span class="release-tag">СЕРВЕРНАЯ СБОРКА</span>
					</div>
				</section>
				<LibraryPage
					v-else-if="page === 'library'"
					:detail="routeDetail"
					:settings="snapshot.settings"
					:account-name="activeAccount?.nickname || null"
					:operations="operations"
				/>
				<ContentPage
					v-else-if="page === 'content'"
					:detail="routeDetail"
					:operations="operations"
					:release-channel="snapshot.settings.release_channel || 'stable'"
				/>
				<section v-else-if="page === 'accounts'" class="page accounts-page">
					<div class="page-heading">
						<div>
							<p class="eyebrow">ТВОЯ ИГРОВАЯ ИДЕНТИЧНОСТЬ</p>
							<h1>
								Аккаунты<span class="heading-count">{{ snapshot.accounts.length }}</span>
							</h1>
							<p class="page-intro">Один лаунчер. Все твои Minecraft-профили.</p>
						</div>
						<button
							class="button secondary"
							:disabled="busy || authenticating"
							@click="showOffline"
						>
							<AppIcon name="plus" />Офлайн аккаунт
						</button>
					</div>
					<div class="auth-panel">
						<div class="microsoft-mark" aria-hidden="true"><i /><i /><i /><i /></div>
						<div>
							<h2>Твой Minecraft, официально</h2>
							<p>Войди через Microsoft, чтобы добавить лицензионный аккаунт.</p>
						</div>
						<button
							class="button primary"
							:disabled="authenticating || busy || !online"
							@click="microsoftLogin"
						>
							Войти через Microsoft<AppIcon name="arrow" :size="18" />
						</button>
					</div>
					<div class="auth-panel ely-provider-panel">
						<div class="ely-provider-icon"><AppIcon name="shield" :size="22" /></div>
						<div>
							<h2>Аккаунт Ely.by</h2>
							<p>Игровой профиль и скины официальной системы Ely.by.</p>
						</div>
						<button
							class="button secondary"
							:disabled="authenticating || busy || !online"
							@click="showElyLogin"
						>
							Войти в Ely.by<AppIcon name="arrow" :size="17" />
						</button>
					</div>
					<div
						v-if="authenticating && authProvider === 'microsoft'"
						class="auth-progress"
						role="status"
					>
						<span class="spinner" />
						<div>
							<strong>{{
								authStage === 'minecraft'
									? 'Проверяем профиль Minecraft…'
									: 'Ожидаем вход через Microsoft…'
							}}</strong>
							<p>
								{{
									authStage === 'minecraft'
										? 'Вход завершён. Проверяем игровой профиль и сохраняем аккаунт…'
										: 'Заверши авторизацию в открывшемся браузере. Пароль остаётся у Microsoft.'
								}}
							</p>
						</div>
						<button
							class="button subtle"
							:disabled="authStage === 'minecraft'"
							@click="cancelLogin"
						>
							Отменить
						</button>
					</div>
					<div v-if="snapshot.accounts.length === 0" class="empty-state account-empty">
						<div class="empty-portrait">
							<span class="pixel-avatar large" /><span class="floating-plus">+</span>
						</div>
						<h2>Здесь начинается твоя история</h2>
						<p>
							Добавь аккаунт Microsoft или создай локальный<br />профиль — для него нужен только
							никнейм.
						</p>
						<button
							class="button secondary"
							:disabled="busy || authenticating"
							@click="showOffline"
						>
							<AppIcon name="plus" />Добавить офлайн аккаунт</button
						><small>Скины офлайн профилей — из Ely.by, без пароля.</small>
					</div>
					<div v-else class="account-grid">
						<button
							v-for="account in snapshot.accounts"
							:key="account.uuid"
							class="account-card"
							:class="{ selected: account.active }"
							@click="showAccount(account)"
						>
							<span class="account-card-top"
								><span class="account-kind">{{ accountType(account) }}</span
								><span v-if="account.active" class="active-badge"><span />Активный</span></span
							><span class="account-portrait"
								><img
									v-if="skins[account.uuid]?.head"
									:src="skins[account.uuid]?.head || ''"
									width="72"
									height="72"
									:alt="`Аватар ${account.nickname}`" /><span
									v-else
									class="pixel-avatar large" /><span
									v-if="skinLoading[account.uuid]"
									class="avatar-loader spinner" /></span
							><strong>{{ account.nickname }}</strong
							><span class="account-uuid">{{ account.uuid }}</span
							><span class="account-card-bottom"
								><span>{{
									skinLoading[account.uuid]
										? 'Загружаем скин…'
										: skins[account.uuid]?.status === 'network_error'
											? 'Скин недоступен · резервный аватар'
											: skins[account.uuid]?.provider === 'ely_by'
												? 'Скин Ely.by'
												: account.kind !== 'offline'
													? 'Профиль Minecraft'
													: 'Локальный профиль'
								}}</span
								><AppIcon name="arrow" :size="16"
							/></span>
						</button>
					</div>
					<div class="privacy-note">
						<AppIcon name="shield" :size="18" />
						<p>
							Аккаунты хранятся на этом устройстве. Офлайн профиль не подтверждает владение
							Minecraft.
						</p>
					</div>
				</section>
				<section v-else-if="draft" class="page settings-page">
					<div class="page-heading">
						<div>
							<p class="eyebrow">НАСТРОЙ ПОД СЕБЯ</p>
							<h1>Настройки</h1>
							<p class="page-intro">Комфорт начинается с деталей.</p>
						</div>
						<button
							class="button primary"
							:disabled="busy || !settingsChanged"
							@click="saveSettings"
						>
							{{ busy ? 'Сохраняем…' : 'Сохранить изменения' }}<AppIcon name="check" :size="17" />
						</button>
					</div>
					<div class="settings-section">
						<h2><AppIcon name="settings" />Лаунчер</h2>
						<div class="setting-row">
							<div>
								<label for="language">Язык интерфейса</label>
								<p>Основной язык первого этапа</p>
							</div>
							<select id="language" v-model="draft.locale">
								<option value="ru">Русский</option>
								<option disabled>English — скоро</option>
							</select>
						</div>
						<div class="setting-row">
							<div>
								<strong>Запуск вместе с системой</strong>
								<p>
									{{
										autostartAvailable
											? 'Открывать NCreate при входе в систему'
											: 'Недоступно в текущей среде'
									}}
								</p>
							</div>
							<button
								class="switch"
								:class="{ on: autostart }"
								role="switch"
								:aria-checked="autostart"
								aria-label="Запуск вместе с системой"
								:disabled="!autostartAvailable"
								@click="toggleAutostart"
							>
								<span />
							</button>
						</div>
					</div>
					<div class="settings-section launcher-updates-section">
						<h2><AppIcon name="refresh" />Обновления лаунчера</h2>
						<div class="setting-row">
							<div>
								<strong>Автоматическая проверка</strong>
								<p>Искать обновления при запуске. Установка начнётся только после подтверждения.</p>
							</div>
							<button
								class="switch"
								:class="{ on: draft.auto_updates }"
								role="switch"
								:aria-checked="draft.auto_updates"
								aria-label="Автоматическая проверка обновлений"
								@click="draft.auto_updates = !draft.auto_updates"
							>
								<span />
							</button>
						</div>
						<div class="setting-row">
							<div>
								<label for="release-channel">Канал версий</label>
								<p>Stable не получает Beta автоматически. Сохраните выбор перед проверкой.</p>
							</div>
							<select id="release-channel" v-model="draft.release_channel">
								<option value="stable">Stable · стабильный</option>
								<option value="beta">Beta · предварительный</option>
							</select>
						</div>
						<div class="setting-row launcher-update-check-row">
							<div>
								<strong>Текущая версия — {{ version }}</strong>
								<p>Канал: {{ snapshot.settings.release_channel === 'beta' ? 'Beta' : 'Stable' }}</p>
							</div>
							<button
								class="button secondary"
								:disabled="
									!online ||
									settingsChanged ||
									['checking', 'downloading', 'installing', 'restarting'].includes(
										launcherUpdates.phase.value,
									)
								"
								@click="launcherUpdates.check(snapshot.settings.release_channel)"
							>
								<AppIcon name="refresh" :size="17" />{{
									launcherUpdates.phase.value === 'checking' ? 'Проверяем…' : 'Проверить обновления'
								}}
							</button>
						</div>
						<div
							v-if="settingsChanged || launcherUpdates.phase.value !== 'idle'"
							class="launcher-update-state"
							aria-live="polite"
						>
							<p v-if="settingsChanged" class="field-help">
								Сохраните настройки, чтобы применить выбранный канал обновлений.
							</p>
							<p v-if="launcherUpdates.phase.value === 'checking'" role="status">
								<span class="spinner" />Проверка обновлений…
							</p>
							<p v-else-if="launcherUpdates.phase.value === 'current'" role="status">
								<AppIcon name="check" :size="17" /> Установлена последняя доступная версия.
							</p>
							<div
								v-else-if="launcherUpdates.phase.value === 'available'"
								class="launcher-update-available"
							>
								<strong>Доступно обновление {{ launcherUpdates.candidate.value?.version }}</strong>
								<p>
									Размер: {{ formatUpdateBytes(launcherUpdates.candidate.value?.size_bytes) }} ·
									Подпись будет проверена перед установкой.
								</p>
								<details v-if="launcherUpdates.candidate.value?.notes">
									<summary>Что нового</summary>
									<!-- Release notes pass through the existing tag allowlist and DOMPurify. -->
									<!-- eslint-disable vue/no-v-html -->
									<div
										class="launcher-update-notes project-markdown"
										v-html="launcherUpdateNotes"
									/>
									<!-- eslint-enable vue/no-v-html -->
								</details>
								<div class="launcher-update-actions">
									<button class="button primary" @click="launcherUpdates.install()">
										Обновить
									</button>
									<button class="button subtle" @click="launcherUpdates.later()">Позже</button>
								</div>
							</div>
							<div
								v-else-if="
									['downloading', 'installing', 'restarting'].includes(launcherUpdates.phase.value)
								"
								class="launcher-update-progress"
								role="status"
							>
								<strong>{{
									launcherUpdates.phase.value === 'downloading'
										? 'Загрузка обновления'
										: launcherUpdates.phase.value === 'installing'
											? 'Установка обновления'
											: 'Перезапуск лаунчера'
								}}</strong>
								<progress
									v-if="launcherUpdates.progress.value?.total_bytes"
									:max="launcherUpdates.progress.value.total_bytes"
									:value="launcherUpdates.progress.value.downloaded_bytes"
								/>
								<p v-if="launcherUpdates.phase.value === 'downloading'">
									{{ formatUpdateBytes(launcherUpdates.progress.value?.downloaded_bytes) }} /
									{{ formatUpdateBytes(launcherUpdates.progress.value?.total_bytes) }}
									<span v-if="launcherUpdates.progress.value?.bytes_per_second">
										·
										{{ formatUpdateBytes(launcherUpdates.progress.value.bytes_per_second) }}/с</span
									>
								</p>
								<p v-else>
									{{ launcherUpdates.progress.value?.message || 'Пожалуйста, подождите…' }}
								</p>
							</div>
							<div
								v-else-if="launcherUpdates.phase.value === 'error'"
								class="banner error"
								role="alert"
							>
								{{ launcherUpdates.error.value }}
							</div>
						</div>
					</div>
					<div class="settings-section">
						<h2><AppIcon name="moon" />Внешний вид</h2>
						<div class="setting-row">
							<div>
								<label for="theme">Тема</label>
								<p>Тёмная палитра с тёплым акцентом</p>
							</div>
							<select id="theme" v-model="draft.theme">
								<option value="dark">Тёмная</option>
								<option value="oled">Глубокий чёрный</option>
							</select>
						</div>
						<div
							v-for="setting in [
								{
									key: 'animations' as const,
									label: 'Плавные анимации',
									detail: 'Мягкие переходы между состояниями',
								},
								{
									key: 'blur' as const,
									label: 'Прозрачность и размытие',
									detail: 'Лёгкая глубина полупрозрачных поверхностей',
								},
								{
									key: 'reduced_motion' as const,
									label: 'Уменьшить движение',
									detail: 'Отключить необязательные анимации',
								},
							]"
							:key="setting.key"
							class="setting-row"
						>
							<div>
								<strong>{{ setting.label }}</strong>
								<p>{{ setting.detail }}</p>
							</div>
							<button
								class="switch"
								:class="{ on: draft[setting.key] }"
								role="switch"
								:aria-checked="draft[setting.key]"
								:aria-label="setting.label"
								@click="draft[setting.key] = !draft[setting.key]"
							>
								<span />
							</button>
						</div>
					</div>
					<div class="settings-section">
						<h2><AppIcon name="game" />Minecraft</h2>
						<div class="future-note">
							RAM и путь к Java используются при создании своих сборок. Настройки существующих
							профилей сохраняются отдельно.
						</div>
						<div class="setting-row memory-row">
							<div>
								<label for="memory">Оперативная память</label>
								<p>Объём RAM по умолчанию для новых сборок</p>
							</div>
							<div class="memory-control">
								<output for="memory">{{ (draft.memory_mb / 1024).toFixed(1) }} ГБ</output
								><input
									id="memory"
									v-model.number="draft.memory_mb"
									type="range"
									min="1024"
									max="32768"
									step="512"
								/>
							</div>
						</div>
						<div class="setting-row stacked">
							<label for="java">Путь к Java</label
							><input
								id="java"
								v-model="draft.java_path"
								name="java-path"
								autocomplete="off"
								spellcheck="false"
								placeholder="Автоматически после установки сборки"
							/>
						</div>
						<div class="setting-row stacked">
							<label for="game-directory">Папка игры · будущая настройка</label
							><input
								id="game-directory"
								:value="`${snapshot.data_dir}/instances`"
								readonly
								name="game-directory"
								autocomplete="off"
								spellcheck="false"
								aria-describedby="game-directory-help"
							/>
							<p id="game-directory-help" class="field-help">
								Сборки хранятся в отдельной папке NCreate. Выбор другой папки появится вместе с
								безопасным переносом данных.
							</p>
						</div>
					</div>
					<div class="settings-section compact-section">
						<h2><AppIcon name="folder" />Данные приложения</h2>
						<p class="data-directory">{{ snapshot.data_dir }}</p>
						<div class="settings-bottom">
							<button class="button subtle" @click="aboutDialog?.showModal()">
								<AppIcon name="info" :size="17" />О приложении и лицензии</button
							><button class="button subtle" :disabled="busy || authenticating" @click="restart">
								<AppIcon name="refresh" :size="17" />Перезапустить
							</button>
						</div>
					</div>
				</section>
			</template>
		</main>
		<dialog
			ref="offlineDialog"
			class="modal offline-modal"
			aria-labelledby="offline-title"
			@close="nicknameError = ''"
		>
			<button
				class="modal-close icon-button"
				aria-label="Закрыть окно"
				:disabled="busy"
				@click="offlineDialog?.close()"
			>
				<AppIcon name="close" />
			</button>
			<div class="modal-symbol"><AppIcon name="accounts" :size="28" /></div>
			<p class="eyebrow">ТОЛЬКО ТВОЙ НИКНЕЙМ</p>
			<h2 id="offline-title">Офлайн аккаунт</h2>
			<p class="modal-intro">
				Создай локальный Minecraft-профиль.<br />Никаких паролей — просто будь собой.
			</p>
			<form @submit.prevent="addOffline">
				<label for="nickname">Никнейм</label
				><input
					id="nickname"
					ref="nicknameInput"
					v-model="nickname"
					name="nickname"
					autocomplete="off"
					spellcheck="false"
					maxlength="16"
					placeholder="Например, NCreatePlayer"
					:aria-invalid="!!nicknameError"
					aria-describedby="nickname-help"
					@input="nicknameError = ''"
				/>
				<p
					id="nickname-help"
					class="field-help"
					:class="{ 'field-error': nicknameError }"
					aria-live="polite"
				>
					{{ nicknameError || '3–16 символов: латинские буквы, цифры и _' }}
				</p>
				<div v-if="error" class="banner error" role="alert">{{ error }}</div>
				<div class="skin-note">
					<AppIcon name="shield" :size="20" />
					<p>Если у этого никнейма есть скин Ely.by,<br />мы покажем его автоматически.</p>
				</div>
				<button class="button primary full-width" type="submit" :disabled="busy">
					{{ busy ? 'Создаём аккаунт…' : 'Добавить аккаунт' }}<AppIcon name="plus" :size="18" />
				</button>
			</form>
		</dialog>
		<dialog
			ref="accountDialog"
			class="modal account-modal"
			aria-labelledby="account-title"
			@close="deleting = false"
		>
			<template v-if="selectedAccount">
				<button
					class="modal-close icon-button"
					aria-label="Закрыть окно"
					:disabled="busy"
					@click="accountDialog?.close()"
				>
					<AppIcon name="close" />
				</button>
				<div class="profile-hero">
					<img
						v-if="skins[selectedUuid]?.head"
						:src="skins[selectedUuid]?.head || ''"
						width="88"
						height="88"
						alt="Аватар аккаунта"
					/><span v-else class="pixel-avatar large" /><span
						v-if="selectedAccount.active"
						class="active-badge"
						><span />Активный аккаунт</span
					>
				</div>
				<h2 id="account-title">{{ selectedAccount.nickname }}</h2>
				<p class="profile-type">{{ accountType(selectedAccount) }}</p>
				<div class="uuid-field">
					<span>UUID</span><code>{{ selectedAccount.uuid }}</code>
				</div>
				<div v-if="skins[selectedUuid]?.texture" class="skin-preview">
					<img
						:src="skins[selectedUuid]?.texture || ''"
						width="192"
						height="192"
						alt="Развёртка скина Minecraft"
					/>
					<div>
						<strong>{{
							skins[selectedUuid]?.provider === 'fallback' ? 'Стандартный скин' : 'Твой скин'
						}}</strong>
						<p>
							{{
								skins[selectedUuid]?.provider === 'ely_by'
									? 'Ely.by Skin System'
									: skins[selectedUuid]?.provider === 'fallback'
										? 'Steve · локальный резервный скин'
										: 'Minecraft / Mojang'
							}}
						</p>
					</div>
				</div>
				<p v-else class="skin-fallback">
					{{
						skinLoading[selectedUuid]
							? 'Загружаем скин…'
							: 'Скин не найден или сеть недоступна. Используем резервный аватар.'
					}}
				</p>
				<form
					v-if="selectedAccount.kind === 'offline'"
					class="rename-form"
					@submit.prevent="renameAccount"
				>
					<label for="rename">Никнейм</label>
					<div class="rename-row">
						<input
							id="rename"
							v-model="editNickname"
							name="rename"
							autocomplete="off"
							spellcheck="false"
							maxlength="16"
						/><button
							class="button secondary"
							:disabled="busy || editNickname === selectedAccount.nickname"
						>
							Сохранить
						</button>
					</div>
					<p class="field-help">При смене никнейма изменится и офлайн UUID.</p>
				</form>
				<div v-if="error" class="banner error" role="alert">{{ error }}</div>
				<div class="profile-actions">
					<button
						v-if="!selectedAccount.active"
						class="button primary full-width"
						:disabled="busy"
						@click="mutate('set_active', { uuid: selectedUuid }, 'Активный аккаунт изменён')"
					>
						<AppIcon name="check" :size="17" />Сделать активным</button
					><button
						v-if="selectedAccount.kind !== 'offline'"
						class="button secondary full-width"
						:disabled="busy || !online"
						@click="refreshAccount"
					>
						<AppIcon name="refresh" :size="17" />Обновить данные профиля</button
					><button
						v-else
						class="button subtle full-width"
						:disabled="skinLoading[selectedUuid] || !online"
						@click="loadSkin(selectedAccount, true)"
					>
						<AppIcon name="refresh" :size="17" />Обновить скин
					</button>
					<div v-if="deleting" class="delete-confirm">
						<p>
							Удалить аккаунт из лаунчера?<br /><small>Это не удалит сам Minecraft-аккаунт.</small>
						</p>
						<div>
							<button class="button subtle" :disabled="busy" @click="deleting = false">
								Отмена</button
							><button class="button danger" :disabled="busy" @click="deleteAccount">
								{{ busy ? 'Удаляем…' : 'Удалить' }}
							</button>
						</div>
					</div>
					<button
						v-else
						class="button delete-button full-width"
						:disabled="busy"
						@click="deleting = true"
					>
						<AppIcon name="trash" :size="16" />Удалить аккаунт
					</button>
				</div>
			</template>
		</dialog>
		<dialog ref="aboutDialog" class="modal about-modal" aria-labelledby="about-title">
			<button
				class="modal-close icon-button"
				aria-label="Закрыть окно"
				@click="aboutDialog?.close()"
			>
				<AppIcon name="close" /></button
			><img
				class="about-logo"
				src="/brand/logo.webp"
				width="72"
				height="72"
				alt="Логотип NCreate"
			/>
			<h2 id="about-title">NCreate Launcher</h2>
			<p class="profile-type">Версия {{ version }} · Библиотека и контент</p>
			<p>Официальный лаунчер проекта NCreate.</p>
			<p>
				Основан на открытом desktop-коде Modrinth App (Theseus). Copyright © Modrinth и участники
				проекта. Изменения © 2026 NCreate. Распространяется по GNU GPL v3.
			</p>
			<details>
				<summary>Текст лицензии GNU GPL v3</summary>
				<pre class="license-frame">{{ license || 'Загружаем лицензию…' }}</pre>
			</details>
			<details>
				<summary>Лицензии шрифтов Manrope и Unbounded</summary>
				<pre class="license-frame">{{ fontLicenses || 'Загружаем лицензии…' }}</pre>
			</details>
			<p class="legal-note">
				Не является официальным продуктом Minecraft. Не одобрено и не связано с Mojang или
				Microsoft.
			</p>
		</dialog>
		<ElyLoginDialog ref="elyLoginDialog" @saved="applySnapshot" @busy="authenticating = $event" />
	</div>
</template>
