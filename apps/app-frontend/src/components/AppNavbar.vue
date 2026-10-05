<script setup lang="ts">
import { ref } from 'vue'
import AppIcon from './AppIcon.vue'
import type { Account, Skin } from '../models'
import type { Page } from '../routes'
const props = defineProps<{
	page: Page
	navigation: { id: Page; label: string; icon: string }[]
	accounts: Account[]
	activeAccount: Account | null | undefined
	skins: Record<string, Skin | undefined>
	version: string
}>()
const emit = defineEmits<{
	navigate: [page: Page]
	account: [account: Account]
	activate: [account: Account]
	about: []
}>()
const menu = ref<HTMLDialogElement | null>(null)
function activate(account: Account) {
	emit('activate', account)
	menu.value?.close()
}
function navigate(page: Page) {
	menu.value?.close()
	emit('navigate', page)
}
</script>
<template>
	<header class="app-navbar">
		<a href="#/home" class="navbar-brand" aria-label="NCreate — Главная" @click="navigate('home')">
			<img src="/brand/logo.webp" width="32" height="32" alt="" />
			<span>NCreate <small>Launcher</small></span>
		</a>
		<nav aria-label="Основная навигация">
			<a
				v-for="item in navigation.filter((entry) => !['accounts', 'settings'].includes(entry.id))"
				:key="item.id"
				:href="`#/${item.id}`"
				:class="{ active: page === item.id }"
				:aria-current="page === item.id ? 'page' : undefined"
				@click="navigate(item.id)"
			>
				{{ item.label }}
			</a>
		</nav>
		<div class="navbar-tools">
			<button
				class="icon-button"
				title="О приложении"
				aria-label="О приложении"
				@click="emit('about')"
			>
				<AppIcon name="info" :size="18" />
			</button>
			<a
				href="#/settings"
				class="icon-button"
				:class="{ active: page === 'settings' }"
				aria-label="Настройки"
				title="Настройки"
				@click="navigate('settings')"
				><AppIcon name="settings" :size="20"
			/></a>
			<button
				class="navbar-account"
				aria-label="Переключить аккаунт"
				aria-haspopup="dialog"
				@click="menu?.showModal()"
			>
				<img
					v-if="activeAccount && skins[activeAccount.uuid]?.head"
					:src="skins[activeAccount.uuid]?.head || ''"
					width="32"
					height="32"
					alt=""
				/>
				<AppIcon v-else name="accounts" :size="22" />
				<span>{{ activeAccount?.nickname || 'Аккаунты' }}</span
				><AppIcon name="chevron" :size="14" />
			</button>
		</div>
		<dialog
			ref="menu"
			class="account-switcher"
			aria-labelledby="switcher-title"
			@click="$event.target === menu && menu?.close()"
		>
			<div class="account-switcher-heading">
				<h2 id="switcher-title">Аккаунты</h2>
				<button class="icon-button" aria-label="Закрыть" @click="menu?.close()">
					<AppIcon name="close" :size="16" />
				</button>
			</div>
			<button
				v-for="account in props.accounts"
				:key="account.uuid"
				class="switcher-account"
				:class="{ active: account.active }"
				@click="activate(account)"
			>
				<img
					v-if="skins[account.uuid]?.head"
					:src="skins[account.uuid]?.head || ''"
					width="32"
					height="32"
					alt=""
				/><AppIcon v-else name="accounts" />
				<span
					>{{ account.nickname
					}}<small>{{
						account.kind === 'offline'
							? 'Офлайн'
							: account.kind === 'ely_by'
								? 'Ely.by'
								: 'Microsoft'
					}}</small></span
				><AppIcon v-if="account.active" name="check" :size="16" />
			</button>
			<p v-if="!accounts.length" class="field-help">Добавьте профиль, чтобы начать играть.</p>
			<button class="button secondary full-width" @click="navigate('accounts')">
				<AppIcon name="plus" :size="16" />Управление аккаунтами
			</button>
			<small class="switcher-version">NCreate Launcher {{ version }}</small>
		</dialog>
	</header>
</template>
