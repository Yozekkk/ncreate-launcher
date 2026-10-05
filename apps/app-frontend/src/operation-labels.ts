import type { Progress } from './game-api'
export function operationTitle(operation: string): string {
	const labels: Record<string, string> = {
		java: 'Подготавливаем Java',
		install_game: 'Устанавливаем Minecraft',
		install_content: 'Устанавливаем моды',
		install_modpack: 'Устанавливаем модпак',
		update_content: 'Обновляем моды',
		import_pack: 'Импортируем сборку',
		install_edition: 'Устанавливаем NCreate',
		update_edition: 'Обновляем NCreate',
		resume_edition: 'Завершаем установку NCreate',
	}
	return labels[operation] || 'Подготавливаем файлы'
}
export function operationMessage(job: Progress): string {
	if (job.phase === 'java') return job.message
	if (job.phase === 'downloading')
		return `Загружаем файлы · ${(job.completed / 1048576).toFixed(1)}${job.total > 0 ? ' / ' + (job.total / 1048576).toFixed(1) : ''} МБ${['install_edition', 'update_edition', 'resume_edition'].includes(job.operation) && job.message.startsWith('Осталось файлов:') ? ` · ${job.message}` : ''}`
	if (job.phase === 'committing') return 'Проверка завершена. Сохраняем изменения…'
	if (job.phase === 'loader_processors') return 'Настраиваем загрузчик Minecraft…'
	if (job.phase === 'checking') return 'Проверяем кеш Minecraft…'
	if (job.phase === 'verifying') return 'Проверяем целостность файлов…'
	return 'Подготавливаем установку и проверяем совместимость…'
}
