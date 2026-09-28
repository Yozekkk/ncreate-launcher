export type Page = 'home' | 'library' | 'content' | 'accounts' | 'settings'
export type LauncherRoute = { page: Page; detail: string | null }

export function parseLauncherRoute(hash: string): LauncherRoute {
	const parts =
		hash
			.replace(/^#?\/?/, '')
			.split('?')[0]
			?.split('/') || []
	const page = parts[0]
	if (page === 'library' || page === 'content') {
		const detail = parts[1]
		return { page, detail: detail && /^[A-Za-z0-9_-]{1,128}$/.test(detail) ? detail : null }
	}
	if (page === 'accounts' || page === 'settings') return { page, detail: null }
	return { page: 'home', detail: null }
}

export function formatCount(value: number): string {
	return new Intl.NumberFormat('ru', {
		notation: value >= 10000 ? 'compact' : 'standard',
		maximumFractionDigits: 1,
	}).format(value)
}
