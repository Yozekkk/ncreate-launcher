import test from 'node:test'
import assert from 'node:assert/strict'
import { parseLauncherRoute, formatCount } from '../src/routes.ts'
import { compatibleVersions, statusLabel } from '../src/game-api.ts'

const instance = { id: 'local', game_version: '1.21.1', loader: 'fabric' }
const versions = [
	{ id: 'match', game_versions: ['1.21.1'], loaders: ['fabric'] },
	{ id: 'wrong-game', game_versions: ['1.20.1'], loaders: ['fabric'] },
	{ id: 'wrong-loader', game_versions: ['1.21.1'], loaders: ['forge'] },
	{ id: 'multiloader', game_versions: ['1.21', '1.21.1'], loaders: ['quilt', 'fabric'] },
]
test('all product routes are closed to legacy and arbitrary pages', () => {
	for (const page of ['home', 'library', 'content', 'accounts', 'settings'])
		assert.equal(parseLauncherRoute('#/' + page).page, page)
	for (const path of ['browse', 'friends', 'hosting', 'marketplace', 'user', 'instances'])
		assert.deepEqual(parseLauncherRoute('#/' + path), { page: 'home', detail: null })
})
test('library and project detail retain safe identifiers and separate search parameters', () => {
	assert.deepEqual(parseLauncherRoute('#/library/abc-123?tab=mods'), {
		page: 'library',
		detail: 'abc-123',
	})
	assert.deepEqual(parseLauncherRoute('#/content/sodium?q=shader&loader=fabric'), {
		page: 'content',
		detail: 'sodium',
	})
	for (const id of ['..', '%2e%2e', '%2Fetc', '<script>', 'a'.repeat(129)])
		assert.equal(parseLauncherRoute('#/library/' + id).detail, null)
	assert.equal(parseLauncherRoute('#/accounts/unknown').detail, null)
})
test('compatible version selection requires both exact Minecraft version and loader', () => {
	assert.deepEqual(
		compatibleVersions(versions, instance).map((v) => v.id),
		['match', 'multiloader'],
	)
	assert.deepEqual(compatibleVersions(versions, { ...instance, loader: 'vanilla' }), [])
	assert.deepEqual(compatibleVersions(versions, undefined), [])
	assert.deepEqual(
		compatibleVersions(versions, { ...instance, loader: 'forge' }).map((v) => v.id),
		['wrong-loader'],
	)
})
test('user status distinguishes installation, ready, running and recoverable failure', () => {
	assert.equal(statusLabel('created'), 'Нужна установка')
	assert.equal(statusLabel('running'), 'Игра запущена')
	assert.equal(statusLabel('error'), 'Нужна проверка')
	assert.match(formatCount(1500000), /1,5/)
})
