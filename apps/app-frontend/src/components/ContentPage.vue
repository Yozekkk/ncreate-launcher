<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { operationTitle, operationMessage } from '../operation-labels'
import AppIcon from './AppIcon.vue'
import OperationPanel from './OperationPanel.vue'
import {
	gameApi,
	compatibleVersions,
	displayError,
	loaderNames,
	type ContentKind,
	type SearchHit,
	type SearchResult,
	type Project,
	type ProjectVersion,
	type Instance,
	type GameVersion,
	type Loader,
} from '../game-api'
import { operationPercent, type GameOperations } from '../game-operations'
import { formatCount, parseLauncherRoute } from '../routes'
import { renderProjectMarkdown } from '../project-markdown'

const props = defineProps<{
	detail: string | null
	operations: GameOperations
	releaseChannel: 'stable' | 'beta'
}>()
const kind = ref<ContentKind>('mod')
const query = ref('')
const category = ref('')
const gameVersion = ref('')
const loader = ref('')
const sort = ref('relevance')
const offset = ref(0)
const limit = 20
const result = ref<SearchResult>({ hits: [], offset: 0, total_hits: 0 })
const loading = ref(false)
const detailLoading = ref(false)
const busy = ref(false)
const error = ref('')
const notice = ref('')
const categories = ref<string[]>([])
const gameVersions = ref<GameVersion[]>([])
const instances = ref<Instance[]>([])
const project = ref<Project | null>(null)
const versions = ref<ProjectVersion[]>([])
const target = ref('')
const selectedVersion = ref('')
const installName = ref('')
const installDialog = ref<HTMLDialogElement | null>(null)
const selectedHit = ref<SearchHit | null>(null)
let searchGeneration = 0
let detailGeneration = 0
let alive = true
function dismissError() {
	error.value = ''
	props.operations.clearError()
}
function resetFilters() {
	query.value = ''
	category.value = ''
	loader.value = ''
	gameVersion.value = ''
	void search(true)
}
const dates = new Intl.DateTimeFormat('ru', { day: 'numeric', month: 'long', year: 'numeric' })
const updatedAt = (value: string) =>
	value && Number.isFinite(Date.parse(value)) ? dates.format(new Date(value)) : 'Дата не указана'
const targetInstance = computed(() =>
	instances.value.find((instance) => instance.id === target.value),
)
const compatible = computed(() => compatibleVersions(versions.value, targetInstance.value))
const availableVersions = computed(() =>
	(project.value?.project_type === 'modpack' ? versions.value : compatible.value).filter(
		(version) => props.releaseChannel === 'beta' || version.version_type === 'release',
	),
)
const compatibleTargets = computed(() =>
	instances.value.filter(
		(instance) =>
			!['running', 'installing'].includes(instance.status) &&
			compatibleVersions(versions.value, instance).length > 0,
	),
)
const selectedVersionInfo = computed(() =>
	availableVersions.value.find((version) => version.id === selectedVersion.value),
)
const pageNumber = computed(() => Math.floor(offset.value / limit) + 1)
const pages = computed(() => Math.max(1, Math.ceil(result.value.total_hits / limit)))
const projectBody = computed(() => renderProjectMarkdown(project.value?.body || ''))
const loaders: Loader[] = ['fabric', 'forge', 'neoforge', 'quilt']
function syncFilters() {
	const params = new URLSearchParams(location.hash.split('?')[1] || '')
	kind.value = params.get('type') === 'modpack' ? 'modpack' : 'mod'
	query.value = params.get('q') || ''
	category.value = params.get('category') || ''
	gameVersion.value = params.get('version') || ''
	loader.value = params.get('loader') || ''
	sort.value = params.get('sort') || 'relevance'
	offset.value = Math.max(0, Number.parseInt(params.get('offset') || '0', 10) || 0)
	if (params.get('instance')) target.value = params.get('instance') || ''
}
function filterHash(detail: string | null = null) {
	const params = new URLSearchParams()
	if (kind.value === 'modpack') params.set('type', 'modpack')
	if (query.value) params.set('q', query.value)
	if (category.value) params.set('category', category.value)
	if (gameVersion.value) params.set('version', gameVersion.value)
	if (loader.value) params.set('loader', loader.value)
	if (sort.value !== 'relevance') params.set('sort', sort.value)
	if (offset.value) params.set('offset', String(offset.value))
	if (target.value) params.set('instance', target.value)
	return `#/content${detail ? `/${detail}` : ''}${params.size ? `?${params}` : ''}`
}
async function search(reset = false) {
	if (reset) offset.value = 0
	const generation = ++searchGeneration
	loading.value = true
	error.value = ''
	if (!props.detail) history.replaceState(null, '', filterHash())
	try {
		const next = await gameApi.search({
			query: query.value.trim(),
			kind: kind.value,
			category: category.value || null,
			game_version: gameVersion.value || null,
			loader: loader.value || null,
			sort: sort.value,
			channel: props.releaseChannel,
			offset: offset.value,
			limit,
		})
		if (alive && generation === searchGeneration) result.value = next
	} catch (reason) {
		if (alive && generation === searchGeneration) error.value = displayError(reason)
	} finally {
		if (alive && generation === searchGeneration) loading.value = false
	}
}
async function loadTaxonomy() {
	try {
		const values = await gameApi.categories(kind.value)
		if (alive) categories.value = values
	} catch {
		categories.value = []
	}
}
async function setKind(value: ContentKind) {
	if (kind.value === value) return
	kind.value = value
	category.value = ''
	await loadTaxonomy()
	await search(true)
}
async function loadProject(id: string) {
	const generation = ++detailGeneration
	detailLoading.value = true
	error.value = ''
	project.value = null
	versions.value = []
	selectedVersion.value = ''
	try {
		const [nextProject, nextVersions, nextInstances] = await Promise.all([
			gameApi.project(id),
			gameApi.versions(id),
			gameApi.instances(),
		])
		if (!alive || generation !== detailGeneration) return
		project.value = nextProject
		versions.value = nextVersions
		instances.value = nextInstances
		installName.value = nextProject.title
		if (!compatibleTargets.value.some((instance) => instance.id === target.value))
			target.value = compatibleTargets.value[0]?.id || ''
		selectedVersion.value = availableVersions.value[0]?.id || ''
	} catch (reason) {
		if (alive && generation === detailGeneration) error.value = displayError(reason)
	} finally {
		if (alive && generation === detailGeneration) detailLoading.value = false
	}
}
async function prepareInstall(hit: SearchHit) {
	selectedHit.value = hit
	installDialog.value?.showModal()
	await loadProject(hit.project_id)
}
async function install() {
	const currentProject = project.value
	if (!currentProject || !selectedVersion.value || !selectedVersionInfo.value) {
		error.value = 'Выберите совместимую версию проекта.'
		return
	}
	if (currentProject.project_type !== 'modpack' && !targetInstance.value) {
		error.value = 'Выберите сборку для установки.'
		return
	}
	busy.value = true
	error.value = ''
	try {
		const jobId =
			currentProject.project_type === 'modpack'
				? await gameApi.installModpack(
						currentProject.id,
						selectedVersion.value,
						installName.value.trim() || currentProject.title,
					)
				: await gameApi.installContent(
						target.value,
						currentProject.id,
						selectedVersion.value,
						'mod',
					)
		await props.operations.track(jobId)
		installDialog.value?.close()
		notice.value = 'Установка началась. Файлы и зависимости проверяет лаунчер.'
	} catch (reason) {
		error.value = displayError(reason)
	} finally {
		busy.value = false
	}
}
async function changePage(delta: number) {
	offset.value = Math.max(0, offset.value + delta * limit)
	await search()
}
function openInstallDialog() {
	selectedHit.value = null
	installDialog.value?.showModal()
}
function onHash() {
	if (
		!parseLauncherRoute(location.hash).detail &&
		parseLauncherRoute(location.hash).page === 'content'
	) {
		syncFilters()
		void search()
	}
}
watch(target, () => {
	selectedVersion.value = availableVersions.value[0]?.id || ''
})
watch(
	() => props.detail,
	(id) => {
		if (id) void loadProject(id)
		else {
			project.value = null
			selectedHit.value = null
			syncFilters()
			void search()
		}
	},
)
onMounted(() => {
	syncFilters()
	window.addEventListener('hashchange', onHash)
	void loadTaxonomy()
	void gameApi
		.gameVersions()
		.then((values) => {
			if (alive) gameVersions.value = values.filter((value) => value.type === 'release')
		})
		.catch(() => {})
	if (props.detail) void loadProject(props.detail)
	else void search()
})
onUnmounted(() => {
	alive = false
	searchGeneration++
	detailGeneration++
	window.removeEventListener('hashchange', onHash)
})
</script>

<template>
	<section class="page content-page">
		<div class="page-heading">
			<div>
				<p class="eyebrow">БОЛЬШЕ ВОЗМОЖНОСТЕЙ ДЛЯ ТВОЕГО МИРА</p>
				<h1>{{ detail && project ? project.title : 'Контент' }}</h1>
				<p class="page-intro">
					{{
						detail
							? 'Выбери совместимую версию и сборку для установки.'
							: 'Найди моды и готовые сборки для своего Minecraft.'
					}}
				</p>
			</div>
			<a class="button secondary" href="#/library"
				><AppIcon name="folder" :size="18" />Моя библиотека</a
			>
		</div>
		<div
			v-if="(error || operations.error.value) && !installDialog?.open"
			class="banner error inline-banner"
			role="alert"
		>
			{{ error || operations.error.value
			}}<button class="icon-button" aria-label="Закрыть сообщение" @click="dismissError">
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
			v-for="job in operations.active.value"
			:key="job.id"
			:title="operationTitle(job.operation)"
			:message="operationMessage(job)"
			:progress="operationPercent(job)"
			:bytes-per-second="job.phase === 'downloading' ? job.bytes_per_second : undefined"
			:cancellable="job.cancellable"
			:cancelling="operations.cancelling.value === job.id"
			@cancel="operations.cancel(job.id)"
		/>
		<template v-if="!detail">
			<div class="content-tabs" role="tablist" aria-label="Тип контента">
				<button
					role="tab"
					:aria-selected="kind === 'mod'"
					:class="{ active: kind === 'mod' }"
					@click="setKind('mod')"
				>
					Моды</button
				><button
					role="tab"
					:aria-selected="kind === 'modpack'"
					:class="{ active: kind === 'modpack' }"
					@click="setKind('modpack')"
				>
					Модпаки</button
				><span>Данные каталога · Modrinth</span>
			</div>
			<form class="content-search" @submit.prevent="search(true)">
				<label class="sr-only" for="content-query">Поиск контента</label
				><input
					id="content-query"
					v-model="query"
					name="content-query"
					autocomplete="off"
					:placeholder="
						kind === 'mod'
							? 'Найти мод: оптимизация, механизмы, новые миры…'
							: 'Найти готовую сборку…'
					"
				/><button class="button primary"><AppIcon name="arrow" :size="18" />Найти</button>
			</form>
			<div class="search-filters">
				<div>
					<label for="content-version">Minecraft</label
					><select id="content-version" v-model="gameVersion" @change="search(true)">
						<option value="">Все версии</option>
						<option v-for="version in gameVersions" :key="version.id" :value="version.id">
							{{ version.id }}
						</option>
					</select>
				</div>
				<div>
					<label for="content-loader">Загрузчик</label
					><select id="content-loader" v-model="loader" @change="search(true)">
						<option value="">Любой</option>
						<option v-for="item in loaders" :key="item" :value="item">
							{{ loaderNames[item] }}
						</option>
					</select>
				</div>
				<div>
					<label for="content-category">Категория</label
					><select id="content-category" v-model="category" @change="search(true)">
						<option value="">Все категории</option>
						<option v-for="item in categories" :key="item" :value="item">{{ item }}</option>
					</select>
				</div>
				<div>
					<label for="content-sort">Сортировка</label
					><select id="content-sort" v-model="sort" @change="search(true)">
						<option value="relevance">По релевантности</option>
						<option value="downloads">По загрузкам</option>
						<option value="updated">По обновлению</option>
						<option value="newest">Сначала новые</option>
					</select>
				</div>
			</div>
			<div class="search-result-heading">
				<span>{{ loading ? 'Ищем проекты…' : `Найдено: ${formatCount(result.total_hits)}` }}</span
				><span>{{
					kind === 'modpack'
						? 'Готовые сборки для отдельных профилей'
						: 'Совместимость проверяется перед установкой'
				}}</span>
			</div>
			<div v-if="loading" class="section-loading content-loading" role="status">
				<span class="spinner" />Загружаем каталог…
			</div>
			<div v-else-if="error && !result.hits.length" class="empty-state compact-empty">
				<AppIcon name="wifi" :size="35" />
				<h2>Каталог пока недоступен</h2>
				<p>Проверь подключение к сети и попробуй снова.</p>
				<button class="button secondary" @click="search()">
					Повторить<AppIcon name="refresh" :size="16" />
				</button>
			</div>
			<div v-else-if="!result.hits.length" class="empty-state compact-empty">
				<AppIcon name="game" :size="35" />
				<h2>Ничего не нашлось</h2>
				<p>Попробуй другой запрос или убери часть фильтров.</p>
				<button class="button secondary" @click="resetFilters">Сбросить фильтры</button>
			</div>
			<div v-else class="project-grid">
				<article v-for="hit in result.hits" :key="hit.project_id" class="project-card">
					<div class="project-card-header">
						<a :href="filterHash(hit.project_id)" tabindex="-1" aria-hidden="true"
							><img
								v-if="hit.icon_url?.startsWith('https://')"
								:src="hit.icon_url"
								width="54"
								height="54"
								alt=""
								loading="lazy" /><span v-else class="project-icon-fallback"
								><AppIcon name="game" :size="24" /></span
						></a>
						<div>
							<a class="project-title" :href="filterHash(hit.project_id)">{{ hit.title }}</a>
							<p>Автор: {{ hit.author }}</p>
						</div>
					</div>
					<p class="project-description">{{ hit.description }}</p>
					<div class="project-tags">
						<span v-for="tag in hit.categories.slice(0, 3)" :key="tag">{{ tag }}</span
						><span v-if="hit.versions.length">{{
							gameVersion && hit.versions.includes(gameVersion)
								? gameVersion
								: hit.versions[hit.versions.length - 1]
						}}</span
						><span v-if="hit.versions.length > 1" :title="hit.versions.join(', ')"
							>+{{ hit.versions.length - 1 }} версий</span
						>
					</div>
					<div class="project-meta">
						<span>{{ formatCount(hit.downloads) }} загрузок</span
						><span>{{ updatedAt(hit.date_modified) }}</span>
					</div>
					<div class="project-card-actions">
						<a class="button subtle" :href="filterHash(hit.project_id)">Подробнее</a
						><button
							class="button secondary"
							:disabled="busy || detailLoading"
							@click="prepareInstall(hit)"
						>
							<AppIcon name="plus" :size="16" />Установить
						</button>
					</div>
				</article>
			</div>
			<nav v-if="pages > 1 && !loading" class="pagination" aria-label="Страницы каталога">
				<button class="button subtle" :disabled="offset === 0" @click="changePage(-1)">Назад</button
				><span>{{ pageNumber }} / {{ pages }}</span
				><button
					class="button subtle"
					:disabled="offset + limit >= result.total_hits"
					@click="changePage(1)"
				>
					Далее<AppIcon name="arrow" :size="16" />
				</button>
			</nav>
		</template>
		<template v-else>
			<a class="back-link" :href="filterHash()"
				><AppIcon name="arrow" :size="16" />Вернуться к каталогу</a
			>
			<div v-if="detailLoading" class="section-loading" role="status">
				<span class="spinner" />Загружаем проект и версии…
			</div>
			<template v-else-if="project"
				><div class="project-detail-hero">
					<img
						v-if="project.icon_url?.startsWith('https://')"
						:src="project.icon_url"
						width="96"
						height="96"
						alt="Иконка проекта"
					/><span v-else class="project-icon-fallback"><AppIcon name="game" :size="40" /></span>
					<div>
						<span class="quiet-badge">{{
							project.project_type === 'modpack' ? 'Модпак' : 'Мод'
						}}</span>
						<p>{{ project.description }}</p>
						<small
							>{{ formatCount(project.downloads) }} загрузок · Обновлён
							{{ updatedAt(project.updated) }}</small
						>
					</div>
					<button
						class="button primary"
						:disabled="busy || !versions.length"
						@click="openInstallDialog"
					>
						<AppIcon name="plus" :size="18" />Установить
					</button>
				</div>
				<div class="project-detail-layout">
					<div class="project-body">
						<h2>О проекте</h2>
						<!-- Only tag-allowlisted, attribute-free DOMPurify output reaches this surface. -->
						<!-- eslint-disable-next-line vue/no-v-html -->
						<div v-if="projectBody" class="project-markdown" v-html="projectBody" />
						<p v-else>{{ project.description }}</p>
					</div>
					<aside class="project-compatibility">
						<h2>Совместимость</h2>
						<h3>Minecraft</h3>
						<div class="project-tags">
							<span v-for="version in project.game_versions" :key="version">{{ version }}</span>
						</div>
						<h3>Загрузчики</h3>
						<div class="project-tags">
							<span v-for="item in project.loaders" :key="item">{{ item }}</span>
						</div>
						<h3>Версии проекта</h3>
						<ul>
							<li v-for="version in versions.slice(0, 12)" :key="version.id">
								<strong>{{ version.version_number }}</strong
								><span>{{ version.game_versions.join(', ') }}</span
								><small>{{ version.version_type }}</small>
							</li>
						</ul>
					</aside>
				</div></template
			>
		</template>
		<dialog
			ref="installDialog"
			class="modal content-install-modal"
			aria-labelledby="install-content-title"
		>
			<button
				class="modal-close icon-button"
				aria-label="Закрыть установку"
				:disabled="busy"
				@click="installDialog?.close()"
			>
				<AppIcon name="close" />
			</button>
			<p class="eyebrow">ПРОВЕРЕННЫЙ КОНТЕНТ</p>
			<h2 id="install-content-title">
				{{ project?.title || selectedHit?.title || 'Установить проект' }}
			</h2>
			<div v-if="detailLoading" class="section-loading" role="status">
				<span class="spinner" />Проверяем доступные версии…
			</div>
			<form v-else-if="project" @submit.prevent="install">
				<p class="modal-intro">
					{{
						project.project_type === 'modpack'
							? 'Создадим отдельную сборку. Существующие миры и профили останутся на месте.'
							: 'Выбери совместимую сборку. Обязательные зависимости установятся вместе с модом.'
					}}
				</p>
				<template v-if="project.project_type === 'modpack'"
					><label for="modpack-name">Название новой сборки</label
					><input
						id="modpack-name"
						v-model="installName"
						name="modpack-name"
						autocomplete="off"
						maxlength="80"
						required /></template
				><template v-else
					><label for="target-instance">Сборка для установки</label
					><select id="target-instance" v-model="target" :disabled="busy" required>
						<option v-if="!compatibleTargets.length" value="">Нет совместимых сборок</option>
						<option v-for="instance in compatibleTargets" :key="instance.id" :value="instance.id">
							{{ instance.name }} · {{ instance.game_version }} · {{ loaderNames[instance.loader] }}
						</option>
					</select>
					<p v-if="!compatibleTargets.length" class="compatibility-warning">
						В библиотеке нет совместимой сборки. Создай профиль с подходящим Minecraft и
						загрузчиком.
					</p>
					<a
						v-if="!compatibleTargets.length"
						class="button secondary full-width"
						href="#/library"
						@click="installDialog?.close()"
						>Открыть библиотеку</a
					></template
				><label class="field-label-spaced" for="project-version">Версия проекта</label
				><select
					id="project-version"
					v-model="selectedVersion"
					:disabled="busy || !availableVersions.length"
					required
				>
					<option v-if="!availableVersions.length" value="">
						Нет подходящей стабильной версии
					</option>
					<option v-for="version in availableVersions" :key="version.id" :value="version.id">
						{{ version.version_number }} · {{ version.game_versions.join(', ')
						}}{{ version.version_type === 'release' ? '' : ` · ${version.version_type}` }}
					</option>
				</select>
				<p class="field-help">
					{{
						releaseChannel === 'stable'
							? 'Стабильный канал: только release-версии. Beta можно включить вручную в настройках.'
							: 'Beta-канал включён: доступны предварительные версии.'
					}}
				</p>
				<div v-if="selectedVersionInfo" class="dependency-summary">
					<AppIcon name="shield" :size="19" />
					<p>
						Файлы проверяются по хешам.<br />Обязательных зависимостей:
						{{
							selectedVersionInfo.dependencies.filter(
								(dependency) => dependency.dependency_type === 'required',
							).length
						}}
					</p>
				</div>
				<div v-if="error" class="banner error" role="alert">{{ error }}</div>
				<button
					class="button primary full-width"
					:disabled="busy || !selectedVersion || (project.project_type !== 'modpack' && !target)"
				>
					{{
						busy
							? 'Запускаем установку…'
							: project.project_type === 'modpack'
								? 'Установить модпак'
								: 'Установить в сборку'
					}}<AppIcon name="plus" :size="18" />
				</button>
			</form>
			<div v-else-if="error" class="banner error" role="alert">{{ error }}</div>
		</dialog>
	</section>
</template>
