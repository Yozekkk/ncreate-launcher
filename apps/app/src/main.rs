//! NCreate desktop shell with an isolated native IPC and credential boundary.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod core_commands;
#[cfg(debug_assertions)]
#[allow(dead_code, clippy::all)]
mod dev_bridge;
mod launcher_updates;
mod skins;
mod storage;

use ncreate_app_lib::{AuthEngine, ElyAuthEngine};
use ncreate_launcher_core::Engine;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use storage::{Result, Settings, Snapshot, Store};
use tauri::{Emitter, Manager};
use tokio::sync::{Mutex, OnceCell};

struct AppState {
	store: OnceCell<Store>,
	auth: AuthEngine,
	ely_auth: ElyAuthEngine,
	engine: OnceCell<Arc<Engine>>,
	file_grants: Mutex<HashMap<PathBuf, core_commands::FileGrant>>,
	skins: skins::SkinService,
	login: Mutex<()>,
	cancellation: AtomicU64,
	pending_update: Mutex<Option<launcher_updates::PendingUpdate>>,
	update_gate: Mutex<()>,
}
impl AppState {
	async fn engine(&self) -> Result<Arc<Engine>> {
		self.engine
			.get_or_try_init(|| async {
				let store = self.store().await?;
				Engine::open(store.directory.clone(), store.pool.clone())
					.await
					.map_err(core_commands::core_error)
			})
			.await
			.cloned()
	}
	async fn store(&self) -> Result<&Store> {
		self.store.get_or_try_init(Store::new).await
	}
}
fn local(window: &tauri::WebviewWindow) -> Result<()> {
	if window.label() != "main" {
		return Err("Операция недоступна в этом окне".into());
	}
	Ok(())
}
fn launcher_route(url: &url::Url) -> Option<&str> {
	let host = url.host_str()?;
	(url.scheme() == "ncreate"
		&& ["home", "library", "content", "accounts", "settings"].contains(&host)
		&& url.username().is_empty()
		&& url.password().is_none()
		&& url.port().is_none()
		&& url.query().is_none()
		&& url.fragment().is_none()
		&& (url.path().is_empty() || url.path() == "/"))
		.then_some(host)
}
#[tauri::command]
fn initial_route(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<Option<String>> {
	use tauri_plugin_deep_link::DeepLinkExt;
	local(&window)?;
	let urls = app
		.deep_link()
		.get_current()
		.map_err(|_| "Не удалось прочитать ссылку NCreate".to_string())?;
	Ok(urls.and_then(|urls| {
		urls.iter()
			.find_map(|url| launcher_route(url).map(str::to_owned))
	}))
}
#[tauri::command]
async fn snapshot(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
) -> Result<Snapshot> {
	local(&window)?;
	state.store().await?.snapshot().await
}
#[tauri::command]
async fn add_offline(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	nickname: String,
) -> Result<Snapshot> {
	local(&window)?;
	state.store().await?.add_offline(&nickname).await
}
#[tauri::command]
async fn rename_offline(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	uuid: String,
	nickname: String,
) -> Result<Snapshot> {
	local(&window)?;
	let store = state.store().await?;
	let result = store.rename(&uuid, &nickname).await?;
	let _ = tokio::fs::remove_file(store.directory.join("skins").join(format!("{uuid}.png"))).await;
	Ok(result)
}
#[tauri::command]
async fn set_active(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	uuid: String,
) -> Result<Snapshot> {
	local(&window)?;
	state.store().await?.activate(&uuid).await
}
#[tauri::command]
async fn remove_account(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	uuid: String,
) -> Result<Snapshot> {
	local(&window)?;
	let store = state.store().await?;
	let account = store.account(&uuid).await?;
	let _auth_gate = if account.kind != "offline" {
		Some(
			state
				.login
				.try_lock()
				.map_err(|_| "Дождитесь завершения операции с аккаунтом".to_string())?,
		)
	} else {
		None
	};
	if account.kind == "microsoft" {
		state
			.auth
			.remove(uuid::Uuid::parse_str(&uuid).map_err(|_| "Некорректный UUID".to_string())?)
			.await
			.map_err(auth_error)?;
	}
	if account.kind == "ely_by" {
		state
			.ely_auth
			.remove(uuid::Uuid::parse_str(&uuid).map_err(|_| "Некорректный UUID".to_string())?)
			.await
			.map_err(|_| "Не удалось завершить сеанс Ely.by".to_string())?;
	}
	let result = store.remove(&uuid).await?;
	let _ = tokio::fs::remove_file(store.directory.join("skins").join(format!("{uuid}.png"))).await;
	Ok(result)
}
#[tauri::command]
async fn refresh_account(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	uuid: String,
) -> Result<Snapshot> {
	local(&window)?;
	let store = state.store().await?;
	let account = store.account(&uuid).await?;
	if account.kind == "offline" {
		return Err("Локальный профиль не требует обновления сеанса".into());
	}
	let _auth_gate = state
		.login
		.try_lock()
		.map_err(|_| "Дождитесь завершения операции с аккаунтом".to_string())?;
	let id = uuid::Uuid::parse_str(&uuid).map_err(|_| "Некорректный UUID".to_string())?;
	if account.kind == "ely_by" {
		let profile = state
			.ely_auth
			.refresh(id)
			.await
			.map_err(|_| "Не удалось обновить профиль Ely.by. Повторите вход.".to_string())?;
		return store.ely_by(profile, false).await;
	}
	let profile = state.auth.refresh(id).await.map_err(auth_error)?;
	let _ = tokio::fs::remove_file(store.directory.join("skins").join(format!("{uuid}.png"))).await;
	store.microsoft(profile, false).await
}
#[tauri::command]
async fn save_settings(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	settings: Settings,
) -> Result<Snapshot> {
	local(&window)?;
	state.store().await?.settings(settings).await
}
#[tauri::command]
async fn skin(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	uuid: String,
	force: Option<bool>,
) -> Result<skins::Skin> {
	local(&window)?;
	let store = state.store().await?;
	state
		.skins
		.load(&store.account(&uuid).await?, store, force.unwrap_or(false))
		.await
}
#[tauri::command]
fn restart_launcher(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<()> {
	local(&window)?;
	app.request_restart();
	Ok(())
}
#[tauri::command]
async fn cancel_login(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
) -> Result<()> {
	local(&window)?;
	state.cancellation.fetch_add(1, Ordering::SeqCst);
	if let Some(signin) = app.get_webview_window("signin") {
		let _ = signin.close();
	}
	Ok(())
}
fn auth_error(error: ncreate_app_lib::Error) -> String {
	match error {
		ncreate_app_lib::Error::Vault(_) => "Не удалось сохранить токены в системном хранилище. Разблокируйте хранилище ключей и повторите вход.".into(),
		_ => "Не удалось войти через Microsoft. Проверьте подключение, наличие Minecraft Java Edition и повторите вход.".into(),
	}
}
/// Reuses the upstream external WebView OAuth/SISU flow and exact Microsoft callback.
#[tauri::command]
async fn microsoft_login(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
) -> Result<Option<Snapshot>> {
	local(&window)?;
	let _gate = state
		.login
		.try_lock()
		.map_err(|_| "Вход через Microsoft уже открыт".to_string())?;
	let generation = state.cancellation.load(Ordering::SeqCst);
	let flow = state.auth.begin().await.map_err(auth_error)?;
	if state.cancellation.load(Ordering::SeqCst) != generation {
		return Ok(None);
	}
	let url = url::Url::parse(&flow.auth_request_uri)
		.map_err(|_| "Некорректный адрес Microsoft".to_string())?;
	if url.scheme() != "https" || url.host_str() != Some("login.live.com") {
		return Err("Некорректный адрес Microsoft".into());
	}
	let signin = tauri::WebviewWindowBuilder::new(&app, "signin", tauri::WebviewUrl::External(url))
		.title("Вход через Microsoft — NCreate Launcher")
		.inner_size(960.0, 720.0)
		.min_inner_size(540.0, 560.0)
		.center()
		.focused(true)
		.on_navigation(|url| {
			url.scheme() == "https"
				&& url.host_str().is_some_and(|host| {
					host == "login.live.com"
						|| host.ends_with(".live.com")
						|| host == "login.microsoftonline.com"
						|| host.ends_with(".microsoft.com")
						|| host.ends_with(".msauth.net")
						|| host.ends_with(".msftauth.net")
				})
		})
		.build()
		.map_err(|_| "Не удалось открыть окно Microsoft".to_string())?;
	let _cleanup = scopeguard::guard(signin.clone(), |window| {
		let _ = window.close();
	});
	let _ = app.emit_to("main", "auth-progress", "browser");
	let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(600);
	while tokio::time::Instant::now() < deadline {
		if state.cancellation.load(Ordering::SeqCst) != generation || signin.title().is_err() {
			return Ok(None);
		}
		let current = signin
			.url()
			.map_err(|_| "Не удалось прочитать ответ Microsoft".to_string())?;
		if let Some(code) = flow.code_from_redirect(&current).map_err(auth_error)? {
			let _ = signin.close();
			let _ = app.emit_to("main", "auth-progress", "minecraft");
			let profile = state.auth.finish(&code, flow).await.map_err(auth_error)?;
			let store = state.store().await?;
			let existed = store.account(&profile.uuid.to_string()).await.is_ok();
			if state.cancellation.load(Ordering::SeqCst) != generation {
				if !existed {
					state.auth.remove(profile.uuid).await.map_err(auth_error)?;
				}
				return Ok(None);
			}
			let id = profile.uuid;
			let stored = store.microsoft(profile, true).await;
			if stored.is_err() && !existed {
				state.auth.remove(id).await.map_err(auth_error)?;
			}
			return stored.map(Some);
		}
		tokio::time::sleep(std::time::Duration::from_millis(150)).await;
	}
	Err("Время ожидания входа истекло. Попробуйте снова.".into())
}
fn main() {
	tracing_subscriber::fmt()
		.with_max_level(tracing::Level::WARN)
		.init();
	if let Err(error) = run() {
		eprintln!("NCreate Launcher: {error}");
		std::process::exit(1);
	}
}
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let state = AppState {
		store: OnceCell::new(),
		auth: AuthEngine::new(),
		ely_auth: ElyAuthEngine::new(),
		engine: OnceCell::new(),
		file_grants: Mutex::new(HashMap::new()),
		skins: skins::SkinService::new()?,
		login: Mutex::new(()),
		cancellation: AtomicU64::new(0),
		pending_update: Mutex::new(None),
		update_gate: Mutex::new(()),
	};
	let builder = tauri::Builder::default()
		.manage(state)
		.plugin(tauri_plugin_single_instance::init(|app, _, _| {
			if let Some(window) = app.get_webview_window("main") {
				let _ = window.unminimize();
				let _ = window.set_focus();
			}
		}))
		.plugin(tauri_plugin_deep_link::init())
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_opener::init())
		.plugin(tauri_plugin_updater::Builder::new().build())
		.plugin(
			tauri_plugin_window_state::Builder::default()
				.with_state_flags(
					tauri_plugin_window_state::StateFlags::SIZE
						| tauri_plugin_window_state::StateFlags::POSITION
						| tauri_plugin_window_state::StateFlags::MAXIMIZED,
				)
				.build(),
		)
		.plugin(
			tauri_plugin_autostart::Builder::new()
				.app_name("NCreate Launcher")
				.build(),
		)
		.setup(|app| {
			use tauri_plugin_deep_link::DeepLinkExt;
			#[cfg(target_os = "linux")]
			app.deep_link().register_all()?;
			let handle = app.handle().clone();
			app.deep_link().on_open_url(move |event| {
				for url in event.urls() {
					if let Some(route) = launcher_route(&url) {
						let _ = handle.emit_to("main", "ncreate-route", route);
					}
				}
			});
			#[cfg(debug_assertions)]
			{
				dev_bridge::start_bridge(app.handle()).map_err(std::io::Error::other)?;
			}
			Ok(())
		});
	#[cfg(debug_assertions)]
	let builder = builder.invoke_handler(tauri::generate_handler![
		snapshot,
		initial_route,
		add_offline,
		rename_offline,
		set_active,
		remove_account,
		refresh_account,
		save_settings,
		skin,
		restart_launcher,
		microsoft_login,
		core_commands::ely_login,
		core_commands::core_instances,
		core_commands::core_create_instance,
		core_commands::core_edit_instance,
		core_commands::core_delete_instance,
		core_commands::core_duplicate_instance,
		core_commands::core_content,
		core_commands::core_toggle_content,
		core_commands::core_remove_content,
		core_commands::core_check_updates,
		core_commands::core_update_content,
		core_commands::core_rollback,
		core_commands::core_rollback_available,
		core_commands::core_search,
		core_commands::core_project,
		core_commands::core_versions,
		core_commands::core_install_content,
		core_commands::core_install_modpack,
		core_commands::core_install_game,
		core_commands::core_launch,
		core_commands::core_stop,
		core_commands::core_jobs,
		core_commands::core_cancel,
		core_commands::core_game_versions,
		core_commands::core_loader_versions,
		core_commands::core_categories,
		core_commands::core_open_folder,
		core_commands::pick_instance_icon,
		core_commands::pick_import_pack,
		core_commands::core_import_pack,
		core_commands::pick_export_pack,
		core_commands::core_export_pack,
		core_commands::core_editions,
		core_commands::core_install_edition,
		core_commands::core_check_edition_update,
		core_commands::core_apply_edition_update,
		launcher_updates::launcher_check_update,
		launcher_updates::launcher_install_update,
		cancel_login,
		dev_bridge::__dev_bridge_result
	]);
	#[cfg(not(debug_assertions))]
	let builder = builder.invoke_handler(tauri::generate_handler![
		snapshot,
		initial_route,
		add_offline,
		rename_offline,
		set_active,
		remove_account,
		refresh_account,
		save_settings,
		skin,
		restart_launcher,
		microsoft_login,
		core_commands::ely_login,
		core_commands::core_instances,
		core_commands::core_create_instance,
		core_commands::core_edit_instance,
		core_commands::core_delete_instance,
		core_commands::core_duplicate_instance,
		core_commands::core_content,
		core_commands::core_toggle_content,
		core_commands::core_remove_content,
		core_commands::core_check_updates,
		core_commands::core_update_content,
		core_commands::core_rollback,
		core_commands::core_rollback_available,
		core_commands::core_search,
		core_commands::core_project,
		core_commands::core_versions,
		core_commands::core_install_content,
		core_commands::core_install_modpack,
		core_commands::core_install_game,
		core_commands::core_launch,
		core_commands::core_stop,
		core_commands::core_jobs,
		core_commands::core_cancel,
		core_commands::core_game_versions,
		core_commands::core_loader_versions,
		core_commands::core_categories,
		core_commands::core_open_folder,
		core_commands::pick_instance_icon,
		core_commands::pick_import_pack,
		core_commands::core_import_pack,
		core_commands::pick_export_pack,
		core_commands::core_export_pack,
		core_commands::core_editions,
		core_commands::core_install_edition,
		core_commands::core_check_edition_update,
		core_commands::core_apply_edition_update,
		launcher_updates::launcher_check_update,
		launcher_updates::launcher_install_update,
		cancel_login
	]);
	builder.run(tauri::generate_context!())?;
	Ok(())
}

#[cfg(test)]
mod boundary_tests {
	use super::*;

	#[test]
	fn deep_links_only_navigate_known_local_routes() {
		for route in ["home", "library", "content", "accounts", "settings"] {
			let url = url::Url::parse(&format!("ncreate://{route}/")).expect("route URL");
			assert_eq!(launcher_route(&url), Some(route));
		}
		for value in [
			"modrinth://library",
			"ncreate://hosting",
			"ncreate://content/install",
			"ncreate://accounts?code=secret",
			"ncreate://content#install",
			"ncreate://user:secret@home",
			"https://home/",
			"ncreate://home:443/",
		] {
			assert!(launcher_route(&url::Url::parse(value).expect("test URL")).is_none());
		}
	}
}
