import { ref } from 'vue'
export type JavaFailure = {
	instance_id: string
	minecraft: string
	required: number
	message: string
}
export const javaFailure = ref<JavaFailure | null>(null)
export function parseJavaFailure(error: unknown): JavaFailure | null {
	if (typeof error !== 'string') return null
	const start = error.indexOf('NCREATE_JAVA:')
	if (start < 0) return null
	try {
		const value = JSON.parse(error.slice(start + 'NCREATE_JAVA:'.length))
		if (
			typeof value.instance_id !== 'string' ||
			typeof value.minecraft !== 'string' ||
			!Number.isInteger(value.required) ||
			value.required < 1 ||
			typeof value.message !== 'string'
		)
			return null
		return value
	} catch {
		return null
	}
}
export function reportJavaError(error: unknown): JavaFailure | null {
	const failure = parseJavaFailure(error)
	if (failure) javaFailure.value = failure
	return failure
}
