<script setup lang="ts">
import { ref, watch } from 'vue'
import AppIcon from './AppIcon.vue'
import { javaFailure } from '../java-runtime'
import { gameApi, displayError } from '../game-api'
import type { GameOperations } from '../game-operations'
const props = defineProps<{ operations: GameOperations; channel: 'stable' | 'beta' }>()
const dialog = ref<HTMLDialogElement | null>(null)
const busy = ref(false)
const error = ref('')
const selected = ref('')
watch(
	javaFailure,
	(failure) => {
		if (failure) {
			error.value = ''
			selected.value = ''
			dialog.value?.showModal()
		}
	},
	{ flush: 'post' },
)
function close() {
	dialog.value?.close()
	javaFailure.value = null
}
async function pick() {
	const failure = javaFailure.value
	if (!failure) return
	busy.value = true
	error.value = ''
	try {
		const runtime = await gameApi.pickJava(failure.instance_id)
		if (runtime) {
			await gameApi.selectJava(failure.instance_id, runtime.path)
			selected.value = `Java ${runtime.major} · ${runtime.architecture} проверена и сохранена.`
		}
	} catch (reason) {
		error.value = displayError(reason)
	} finally {
		busy.value = false
	}
}
async function retry() {
	const failure = javaFailure.value
	if (!failure) return
	busy.value = true
	error.value = ''
	try {
		const instance = (await gameApi.instances()).find((item) => item.id === failure.instance_id)
		if (!instance) throw new Error('Сборка больше не существует.')
		close()
		if (instance.status === 'ready') await gameApi.launch(instance.id)
		else if (instance.kind === 'official')
			await props.operations.track(await gameApi.resumeEditionInstall(instance.id, props.channel))
		else await props.operations.track(await gameApi.installGame(instance.id))
	} catch (reason) {
		error.value = displayError(reason)
		if (!javaFailure.value) javaFailure.value = failure
		dialog.value?.showModal()
	} finally {
		busy.value = false
	}
}
async function download() {
	if (!javaFailure.value) return
	try {
		await gameApi.javaDownload(javaFailure.value.required)
	} catch (reason) {
		error.value = displayError(reason)
	}
}
</script>
<template>
	<dialog
		ref="dialog"
		class="modal java-runtime-dialog"
		aria-labelledby="java-runtime-title"
		@cancel="close"
	>
		<button class="icon-button modal-close" aria-label="Закрыть" @click="close">
			<AppIcon name="close" />
		</button>
		<div class="modal-symbol"><AppIcon name="info" /></div>
		<h2 id="java-runtime-title">
			Не удалось автоматически подготовить Java {{ javaFailure?.required }}
		</h2>
		<p class="modal-intro">
			Для Minecraft {{ javaFailure?.minecraft }} требуется Java {{ javaFailure?.required }}. NCreate
			Launcher не смог найти или подготовить подходящий runtime.
		</p>
		<p class="java-diagnostic">{{ javaFailure?.message }}</p>
		<p v-if="selected" class="field-help" role="status">{{ selected }}</p>
		<p v-if="error" class="banner error" role="alert">{{ error }}</p>
		<div class="java-actions">
			<button class="button primary" :disabled="busy" @click="download">
				Скачать Java {{ javaFailure?.required }}<AppIcon name="arrow" :size="16" />
			</button>
			<button class="button secondary" :disabled="busy" @click="pick">
				{{ busy ? 'Проверяем Java…' : 'Выбрать Java вручную' }}
			</button>
			<button class="button secondary" :disabled="busy" @click="retry">
				<AppIcon name="refresh" :size="16" />Повторить
			</button>
			<button class="button subtle" @click="close">Отмена</button>
		</div>
		<p class="field-help">
			Загрузка открывает официальный сайт Eclipse Adoptium. Выберите bin/java (Linux) или
			bin/java.exe (Windows). Версия и архитектура проверяются перед сохранением.
		</p>
	</dialog>
</template>
