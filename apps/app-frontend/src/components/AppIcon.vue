<script setup lang="ts">
defineProps<{ name: string; size?: number }>()
const paths: Record<string, string> = {
	home: 'm3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1Z',
	accounts:
		'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M16 3a4 4 0 0 1 0 8 M22 21v-2a4 4 0 0 0-3-3.87 M13 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
	settings:
		'm9 3-.7 2.3-2 .8L4 5.5 2 9l1.8 1.7v2.6L2 15l2 3.5 2.3-.6 2 .8L9 21h4l.7-2.3 2-.8 2.3.6 2-3.5-1.8-1.7v-2.6L20 9l-2-3.5-2.3.6-2-.8L13 3Z M15 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
	plus: 'M12 5v14 M5 12h14',
	arrow: 'M5 12h14 m-5-5 5 5-5 5',
	check: 'm5 12 4 4L19 6',
	close: 'm6 6 12 12 M6 18 18 6',
	shield: 'M12 3 3 7v5c0 5 9 9 9 9s9-4 9-9V7Z m-4 9 3 3 5-6',
	wifi: 'M2 8a16 16 0 0 1 20 0 M5 12a11 11 0 0 1 14 0 M8 16a6 6 0 0 1 8 0 M12 20h.01',
	refresh: 'M20 7v5h-5 M4 17v-5h5 M6 6a8 8 0 0 1 13 3 M18 18a8 8 0 0 1-13-3',
	trash: 'M3 6h18 M9 6V3h6v3 M5 6l1 15h12l1-15 M10 10v7 M14 10v7',
	game: 'M7 7h10c3 0 5 11 3 12-2 1-4-3-5-3H9c-1 0-3 4-5 3-2-1 0-12 3-12Z M7 10v5 M4.5 12.5h5 M16 11h.01 M18 14h.01',
	moon: 'M21 13a9 9 0 1 1-10-10 7 7 0 0 0 10 10',
	folder: 'M3 5h6l2 2h10v13H3Z',
	info: 'M12 16v-4 M12 8h.01 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
}
</script>
<template>
	<svg
		:width="size || 20"
		:height="size || 20"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="1.7"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path :d="paths[name] || paths.info" />
	</svg>
</template>
