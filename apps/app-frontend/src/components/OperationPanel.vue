<script setup lang="ts">
import AppIcon from './AppIcon.vue'
const numbers = new Intl.NumberFormat('ru', { maximumFractionDigits: 1 })
function formatSpeed(bytes: number): string {
	if (bytes >= 1048576) return `${numbers.format(bytes / 1048576)} МБ/с`
	if (bytes >= 1024) return `${numbers.format(bytes / 1024)} КБ/с`
	return `${numbers.format(bytes)} Б/с`
}
defineProps<{
	title: string
	message: string
	progress?: number | null
	bytesPerSecond?: number
	cancellable?: boolean
	cancelling?: boolean
}>()
defineEmits<{ cancel: [] }>()
</script>
<template>
	<div class="operation-panel" role="status" aria-live="polite">
		<span class="spinner" />
		<div class="operation-copy">
			<strong>{{ title }}</strong>
			<p>{{ message }}</p>
			<small v-if="bytesPerSecond && bytesPerSecond > 0" class="operation-speed">{{
				formatSpeed(bytesPerSecond)
			}}</small>
			<progress
				v-if="progress != null"
				:value="progress"
				max="100"
				aria-label="Прогресс операции"
			/>
		</div>
		<button
			v-if="cancellable"
			class="button subtle"
			:disabled="cancelling"
			@click="$emit('cancel')"
		>
			<AppIcon name="close" :size="15" />{{ cancelling ? 'Отменяем…' : 'Отменить' }}
		</button>
	</div>
</template>
