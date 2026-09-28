<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Snapshot } from '../models'
import AppIcon from './AppIcon.vue'

const emit = defineEmits<{ saved: [snapshot: Snapshot]; busy: [value: boolean] }>()
const dialog = ref<HTMLDialogElement | null>(null)
const usernameInput = ref<HTMLInputElement | null>(null)
const username = ref('')
const password = ref('')
const totp = ref('')
const busy = ref(false)
const needsTotp = ref(false)
const error = ref('')
function clearSecrets() {
	password.value = ''
	totp.value = ''
}
async function open() {
	username.value = ''
	clearSecrets()
	error.value = ''
	needsTotp.value = false
	dialog.value?.showModal()
	await nextTick()
	usernameInput.value?.focus()
}
async function close() {
	clearSecrets()
	if (busy.value) {
		try {
			await invoke('cancel_login')
		} catch {
			error.value = 'Не удалось отменить вход. Дождитесь завершения.'
			return
		}
	}
	dialog.value?.close()
}
async function login() {
	if (!username.value.trim() || !password.value) {
		error.value = 'Введите имя пользователя или почту и пароль Ely.by.'
		return
	}
	if (needsTotp.value && !/^\d{6}$/.test(totp.value)) {
		error.value = 'Введите шестизначный код двухфакторной авторизации.'
		return
	}
	busy.value = true
	emit('busy', true)
	error.value = ''
	const request = invoke<Snapshot>('ely_login', {
		username: username.value.trim(),
		password: password.value,
		totp: totp.value || null,
	})
	clearSecrets()
	try {
		const snapshot = await request
		emit('saved', snapshot)
		dialog.value?.close()
		username.value = ''
	} catch (reason) {
		const code = typeof reason === 'string' ? reason : reason instanceof Error ? reason.message : ''
		if (code.includes('ely_two_factor_required')) {
			needsTotp.value = true
			error.value =
				'Нужен код двухфакторной авторизации. Введите пароль повторно и код из приложения.'
		} else if (code.includes('ely_invalid_credentials'))
			error.value = 'Неверные данные Ely.by. Проверьте имя пользователя, пароль и код.'
		else if (code.includes('ely_network'))
			error.value = 'Не удалось связаться с Ely.by. Проверьте подключение и повторите вход.'
		else if (code.includes('cancel')) error.value = 'Вход отменён.'
		else error.value = 'Не удалось войти в Ely.by. Проверьте данные и попробуйте ещё раз.'
	} finally {
		clearSecrets()
		busy.value = false
		emit('busy', false)
	}
}
defineExpose({ open })
</script>
<template>
	<dialog
		ref="dialog"
		class="modal ely-modal"
		aria-labelledby="ely-login-title"
		@close="clearSecrets"
		@cancel.prevent="close"
	>
		<button class="modal-close icon-button" aria-label="Закрыть вход Ely.by" @click="close">
			<AppIcon name="close" />
		</button>
		<div class="modal-symbol"><AppIcon name="shield" :size="28" /></div>
		<p class="eyebrow">ТВОЙ ПРОФИЛЬ ELY.BY</p>
		<h2 id="ely-login-title">Войти в Ely.by</h2>
		<p class="modal-intro">
			Подключи свой игровой аккаунт и скин.<br />Данные проверяет официальный сервис Ely.by.
		</p>
		<form @submit.prevent="login">
			<label for="ely-username">Имя пользователя или почта</label
			><input
				id="ely-username"
				ref="usernameInput"
				v-model="username"
				name="ely-username"
				autocomplete="username"
				spellcheck="false"
				:disabled="busy"
				required
			/>
			<label class="field-label-spaced" for="ely-password">Пароль</label
			><input
				id="ely-password"
				v-model="password"
				name="ely-password"
				type="password"
				autocomplete="off"
				:disabled="busy"
				required
			/>
			<template v-if="needsTotp"
				><label class="field-label-spaced" for="ely-totp">Код двухфакторной авторизации</label
				><input
					id="ely-totp"
					v-model="totp"
					name="ely-totp"
					autocomplete="one-time-code"
					inputmode="numeric"
					maxlength="6"
					:disabled="busy"
					pattern="[0-9]{6}"
					required
			/></template>
			<div v-if="error" class="banner error" role="alert">{{ error }}</div>
			<p class="field-help">
				Пароль используется только для этого входа и не сохраняется. Токен аккаунта хранится в
				защищённом хранилище системы.
			</p>
			<button type="submit" class="button primary full-width" :disabled="busy">
				<span v-if="busy" class="spinner" /><AppIcon v-else name="arrow" :size="18" />{{
					busy ? 'Проверяем аккаунт…' : 'Войти в Ely.by'
				}}
			</button>
			<button v-if="busy" type="button" class="button subtle full-width" @click="close">
				Отменить вход
			</button>
		</form>
	</dialog>
</template>
