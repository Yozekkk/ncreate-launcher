<script setup lang="ts">
import { ref } from 'vue'
import AppIcon from './AppIcon.vue'
const selected = ref(0)
const items = [
	{ label: 'Лаунчер', icon: 'settings' },
	{ label: 'Обновления', icon: 'refresh' },
	{ label: 'Внешний вид', icon: 'moon' },
	{ label: 'Minecraft и Java', icon: 'game' },
	{ label: 'Данные и приложение', icon: 'folder' },
]
function select(index: number) {
	selected.value = index
	document
		.querySelectorAll<HTMLElement>('.settings-section')
		[index]?.scrollIntoView({ block: 'start' })
}
</script>
<template>
	<nav class="settings-sidebar" aria-label="Разделы настроек">
		<p>НАСТРОЙКИ</p>
		<button
			v-for="(item, index) in items"
			:key="item.label"
			:class="{ active: selected === index }"
			@click="select(index)"
		>
			<AppIcon :name="item.icon" :size="18" />{{ item.label }}
		</button>
	</nav>
</template>
