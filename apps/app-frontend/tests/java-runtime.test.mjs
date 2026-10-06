import test from 'node:test'
import assert from 'node:assert/strict'
import { parseJavaFailure, reportJavaError, javaFailure } from '../src/java-runtime.ts'
test('Java recovery retains actual instance, Minecraft and required major', () => {
	const failure = { instance_id: 'instance', minecraft: '1.17', required: 16, message: 'Timeout' }
	const wire = 'Не удалось выполнить операцию: NCREATE_JAVA:' + JSON.stringify(failure)
	assert.deepEqual(parseJavaFailure(wire), failure)
	reportJavaError(wire)
	assert.deepEqual(javaFailure.value, failure)
	for (const value of ['network error', 'NCREATE_JAVA:{}', 'NCREATE_JAVA:bad', null])
		assert.equal(parseJavaFailure(value), null)
})
