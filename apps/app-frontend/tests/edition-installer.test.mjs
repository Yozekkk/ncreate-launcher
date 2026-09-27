import assert from 'node:assert/strict'
import { afterEach, beforeEach, test } from 'node:test'
import {
	canInstallEdition,
	configureEditionInstaller,
	installEdition,
} from '../src/edition-installer.ts'

function edition(manifest = null) {
	return {
		id: 'minimal',
		name: 'Minimal',
		label: '',
		description: '',
		features: [],
		recommended: false,
		manifest,
	}
}

beforeEach(() => configureEditionInstaller(null))
afterEach(() => configureEditionInstaller(null))

test('installation requires both a manifest and an installer', async () => {
	const unavailable = edition()
	assert.equal(canInstallEdition(unavailable), false)
	assert.equal(await installEdition(unavailable), false)
	const configured = edition('test-only-reference')
	assert.equal(canInstallEdition(configured), false)
	assert.equal(await installEdition(configured), false)
	let calls = 0
	configureEditionInstaller(async () => {
		calls += 1
	})
	assert.equal(canInstallEdition(unavailable), false)
	assert.equal(await installEdition(unavailable), false)
	assert.equal(calls, 0)
	assert.equal(canInstallEdition(configured), true)
})

test('empty references cannot enable installation', async () => {
	configureEditionInstaller(async () => assert.fail('installer must not run'))
	for (const reference of ['', '   ']) {
		assert.equal(canInstallEdition(edition(reference)), false)
		assert.equal(await installEdition(edition(reference)), false)
	}
})

test('null configuration disables a previously registered installer', async () => {
	const configured = edition('test-only-reference')
	configureEditionInstaller(async () => {})
	assert.equal(canInstallEdition(configured), true)
	configureEditionInstaller(null)
	assert.equal(canInstallEdition(configured), false)
	assert.equal(await installEdition(configured), false)
})

test('dispatch preserves the selected edition and manifest reference', async () => {
	const received = []
	configureEditionInstaller(async (request) => {
		received.push(request)
	})
	const configured = { ...edition('test-only-reference'), id: 'ultra' }
	assert.equal(await installEdition(configured), true)
	assert.deepEqual(received, [{ editionId: 'ultra', manifest: 'test-only-reference' }])
})

test('explicit cancellation returns false', async () => {
	configureEditionInstaller(async () => false)
	assert.equal(await installEdition(edition('test-only-reference')), false)
})

test('installer errors propagate to the interface error boundary', async () => {
	const reason = new Error('installer test failure')
	configureEditionInstaller(async () => {
		throw reason
	})
	await assert.rejects(installEdition(edition('test-only-reference')), (error) => error === reason)
})
