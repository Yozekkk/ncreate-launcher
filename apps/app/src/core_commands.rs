//! Native adapters validate the invoking window, selected account and file grants.
use crate::{
	AppState, local,
	storage::{Result, Snapshot},
};
use ncreate_app_lib::{GameIdentity, GameSession};
use ncreate_launcher_core::{
	ContentInstall, ContentUpdate, CreateInstance, EditionAvailability, Engine, GameVersion,
	InstalledContent, Instance, LaunchIdentity, Loader, LoaderVersion, Operation, Progress,
	RunningGame, SearchRequest, SearchResult, UpdatePlan,
};
use std::{future::Future, path::PathBuf, sync::Arc};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileGrant {
	Import,
	Export,
}
pub(crate) fn core_error(error: ncreate_launcher_core::Error) -> String {
	match error {
		ncreate_launcher_core::Error::Cancelled => "Операция отменена".into(),
		ncreate_launcher_core::Error::Network(_) => {
			"Сервис недоступен. Проверьте подключение и повторите попытку.".into()
		}
		other => format!("Не удалось выполнить операцию: {other}"),
	}
}
async fn engine(
	window: &tauri::WebviewWindow,
	state: &tauri::State<'_, AppState>,
) -> Result<Arc<Engine>> {
	local(window)?;
	state.engine().await
}
fn job<F, Fut>(
	app: tauri::AppHandle,
	engine: Arc<Engine>,
	kind: &str,
	instance: Option<&str>,
	action: F,
) -> String
where
	F: FnOnce(Arc<Engine>, Operation) -> Fut + Send + 'static,
	Fut: Future<Output = ncreate_launcher_core::Result<()>> + Send + 'static,
{
	let operation = engine.begin(kind, instance);
	let id = operation.id();
	let _ = app.emit_to("main", "core-progress", operation.snapshot());
	tauri::async_runtime::spawn(async move {
		let result = action(engine, operation.clone()).await;
		operation.finish(&result);
		let _ = app.emit_to("main", "core-progress", operation.snapshot());
	});
	id
}
#[tauri::command]
pub async fn ely_login(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	username: String,
	password: String,
	totp: Option<String>,
) -> Result<Snapshot> {
	local(&window)?;
	let password = zeroize::Zeroizing::new(password);
	let totp = totp.map(zeroize::Zeroizing::new);
	let _gate = state
		.login
		.try_lock()
		.map_err(|_| "Дождитесь завершения входа".to_string())?;
	let generation = state.cancellation.load(std::sync::atomic::Ordering::SeqCst);
	let store = state.store().await?;
	let profile = state
		.ely_auth
		.authenticate(
			username,
			password.to_string(),
			totp.as_deref().map(|v| v.to_owned()),
		)
		.await
		.map_err(|e| format!("Вход Ely.by: {e}"))?;
	let existed = store.account(&profile.uuid.to_string()).await.is_ok();
	if state.cancellation.load(std::sync::atomic::Ordering::SeqCst) != generation {
		if !existed {
			state
				.ely_auth
				.remove(profile.uuid)
				.await
				.map_err(|_| "Не удалось закрыть сеанс Ely.by".to_string())?;
		}
		return Err("Вход отменён".into());
	}
	let id = profile.uuid;
	let saved = store.ely_by(profile, true).await;
	if saved.is_err() && !existed {
		state
			.ely_auth
			.remove(id)
			.await
			.map_err(|_| "Не удалось закрыть сеанс Ely.by".to_string())?;
	}
	saved
}
#[tauri::command]
pub async fn core_instances(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
) -> Result<Vec<Instance>> {
	engine(&window, &state)
		.await?
		.instances()
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_create_instance(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	request: CreateInstance,
) -> Result<Instance> {
	engine(&window, &state)
		.await?
		.create_instance(request)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_edit_instance(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	request: CreateInstance,
) -> Result<Instance> {
	engine(&window, &state)
		.await?
		.edit_instance(&instance_id, request)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_delete_instance(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<()> {
	engine(&window, &state)
		.await?
		.delete_instance(&instance_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_duplicate_instance(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	name: String,
) -> Result<Instance> {
	engine(&window, &state)
		.await?
		.duplicate_instance(&instance_id, &name)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_content(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<Vec<InstalledContent>> {
	engine(&window, &state)
		.await?
		.content(&instance_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_toggle_content(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	content_id: String,
	enabled: bool,
) -> Result<()> {
	engine(&window, &state)
		.await?
		.toggle_content(&content_id, enabled)
		.await
		.map(|_| ())
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_remove_content(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	content_id: String,
) -> Result<()> {
	engine(&window, &state)
		.await?
		.remove_content(&content_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_check_updates(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<Vec<ContentUpdate>> {
	let e = engine(&window, &state).await?;
	let channel = state
		.store()
		.await?
		.snapshot()
		.await?
		.settings
		.release_channel;
	e.check_content_updates_for_channel(&instance_id, &channel)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_update_content(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	content_ids: Vec<String>,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	let id = instance_id.clone();
	let channel = state
		.store()
		.await?
		.snapshot()
		.await?
		.settings
		.release_channel;
	Ok(job(
		app,
		e,
		"update_content",
		Some(&id),
		move |e, op| async move {
			e.update_content_for_channel(&instance_id, &content_ids, &channel, &op)
				.await
				.map(|_| ())
		},
	))
}
#[tauri::command]
pub async fn core_rollback(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<()> {
	engine(&window, &state)
		.await?
		.rollback(&instance_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_rollback_available(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<bool> {
	engine(&window, &state)
		.await?
		.rollback_available(&instance_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_search(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	request: SearchRequest,
) -> Result<SearchResult> {
	engine(&window, &state)
		.await?
		.search(request)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_project(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	project_id: String,
) -> Result<serde_json::Value> {
	engine(&window, &state)
		.await?
		.project(&project_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_versions(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	project_id: String,
	game_version: Option<String>,
	loader: Option<String>,
) -> Result<Vec<serde_json::Value>> {
	engine(&window, &state)
		.await?
		.versions(&project_id, game_version.as_deref(), loader.as_deref())
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_install_content(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	request: ContentInstall,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	let id = request.instance_id.clone();
	let channel = state
		.store()
		.await?
		.snapshot()
		.await?
		.settings
		.release_channel;
	Ok(job(
		app,
		e,
		"install_content",
		Some(&id),
		move |e, op| async move {
			e.install_content_for_channel(request, &channel, &op)
				.await
				.map(|_| ())
		},
	))
}
#[tauri::command]
pub async fn core_install_modpack(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	project_id: String,
	version_id: String,
	name: String,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	Ok(job(
		app,
		e,
		"install_modpack",
		None,
		move |e, op| async move {
			e.install_modpack(&project_id, &version_id, &name, &op)
				.await
				.map(|_| ())
		},
	))
}
#[tauri::command]
pub async fn core_install_game(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	let id = instance_id.clone();
	Ok(job(
		app,
		e,
		"install_game",
		Some(&id),
		move |e, op| async move { e.install_game(&instance_id, &op).await.map(|_| ()) },
	))
}
#[tauri::command]
pub async fn core_launch(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<RunningGame> {
	let e = engine(&window, &state).await?;
	let _gate = state
		.login
		.try_lock()
		.map_err(|_| "Дождитесь завершения операции с аккаунтом".to_string())?;
	let account = state
		.store()
		.await?
		.snapshot()
		.await?
		.accounts
		.into_iter()
		.find(|a| a.active)
		.ok_or_else(|| "Выберите активный Minecraft аккаунт".to_string())?;
	let id = uuid::Uuid::parse_str(&account.uuid)
		.map_err(|_| "Некорректный UUID аккаунта".to_string())?;
	let session = match account.kind.as_str() {
		"microsoft" => state.auth.session(id).await.map_err(crate::auth_error)?,
		"ely_by" => state
			.ely_auth
			.session(id)
			.await
			.map_err(|_| "Обновите вход Ely.by перед запуском".to_string())?,
		"offline" => GameSession::offline(GameIdentity {
			uuid: id,
			nickname: account.nickname,
		}),
		_ => return Err("Неизвестный провайдер аккаунта".into()),
	};
	let authlib_injector = if account.kind == "ely_by" {
		let op = e.begin("ely_runtime", Some(&instance_id));
		let _ = app.emit_to("main", "core-progress", op.snapshot());
		let result = e.prepare_ely_runtime(&op).await;
		op.finish(&result);
		let _ = app.emit_to("main", "core-progress", op.snapshot());
		Some(result.map_err(core_error)?)
	} else {
		None
	};
	let identity = LaunchIdentity {
		nickname: session.identity.nickname.clone(),
		uuid: session.identity.uuid.to_string(),
		access_token: session.token().to_owned(),
		user_type: if account.kind == "microsoft" {
			"msa"
		} else {
			"legacy"
		}
		.into(),
		xuid: None,
		authlib_injector,
	};
	let launch = e.launch(&instance_id, identity);
	tokio::pin!(launch);
	let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
	loop {
		tokio::select! {
			result = &mut launch => {
				for progress in e.jobs() {
					let _ = app.emit_to("main", "core-progress", progress);
				}
				return result.map_err(core_error);
			}
			_ = tick.tick() => {
				for progress in e.jobs() {
					let _ = app.emit_to("main", "core-progress", progress);
				}
			}
		}
	}
}
#[tauri::command]
pub async fn core_stop(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<()> {
	engine(&window, &state)
		.await?
		.stop(&instance_id)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_jobs(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
) -> Result<Vec<Progress>> {
	Ok(engine(&window, &state).await?.jobs())
}
#[tauri::command]
pub async fn core_cancel(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	job_id: String,
) -> Result<()> {
	engine(&window, &state)
		.await?
		.cancel(&job_id)
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_game_versions(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
) -> Result<Vec<GameVersion>> {
	engine(&window, &state)
		.await?
		.game_versions()
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_loader_versions(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	loader: Loader,
	game_version: String,
) -> Result<Vec<LoaderVersion>> {
	engine(&window, &state)
		.await?
		.loader_versions(loader, &game_version)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_categories(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	kind: String,
) -> Result<Vec<String>> {
	engine(&window, &state)
		.await?
		.categories(&kind)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_open_folder(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<()> {
	let e = engine(&window, &state).await?;
	let instance = e.instance(&instance_id).await.map_err(core_error)?;
	let store = state.store().await?;
	let path = store.directory.join("instances").join(&instance.id);
	if tokio::fs::canonicalize(&path)
		.await
		.map_err(|_| "Папка сборки недоступна".to_string())?
		!= tokio::fs::canonicalize(&instance.directory)
			.await
			.map_err(|_| "Папка сборки недоступна".to_string())?
	{
		return Err("Некорректная папка сборки".into());
	}
	app.opener()
		.open_path(path.to_string_lossy().into_owned(), None::<&str>)
		.map_err(|_| "Не удалось открыть папку".to_string())
}
async fn pick(app: tauri::AppHandle, save: bool, icon: bool) -> Result<Option<PathBuf>> {
	tokio::task::spawn_blocking(move || {
		let dialog = app.dialog().file().set_title(if icon {
			"Выберите иконку сборки"
		} else if save {
			"Экспорт Minecraft сборки"
		} else {
			"Импорт Minecraft сборки"
		});
		let dialog = if icon {
			dialog.add_filter("PNG", &["png"])
		} else {
			dialog.add_filter("Minecraft modpack", &["mrpack"])
		};
		let selected = if save {
			dialog
				.set_file_name("NCreate-instance.mrpack")
				.blocking_save_file()
		} else {
			dialog.blocking_pick_file()
		};
		selected
			.map(|p| {
				p.into_path()
					.map_err(|_| "Поддерживаются только локальные файлы".to_string())
			})
			.transpose()
	})
	.await
	.map_err(|_| "Не удалось открыть диалог выбора файла".to_string())?
}
#[tauri::command]
pub async fn pick_import_pack(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
) -> Result<Option<String>> {
	local(&window)?;
	let Some(path) = pick(app, false, false).await? else {
		return Ok(None);
	};
	if !path
		.extension()
		.is_some_and(|e| e.eq_ignore_ascii_case("mrpack"))
	{
		return Err("Выберите файл .mrpack".into());
	}
	let path = tokio::fs::canonicalize(path)
		.await
		.map_err(|_| "Выбранный файл недоступен".to_string())?;
	state
		.file_grants
		.lock()
		.await
		.insert(path.clone(), FileGrant::Import);
	Ok(Some(path.to_string_lossy().into_owned()))
}
#[tauri::command]
pub async fn pick_export_pack(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<Option<String>> {
	engine(&window, &state)
		.await?
		.instance(&instance_id)
		.await
		.map_err(core_error)?;
	let Some(mut path) = pick(app, true, false).await? else {
		return Ok(None);
	};
	path.set_extension("mrpack");
	let parent = path
		.parent()
		.ok_or_else(|| "Некорректный путь экспорта".to_string())?;
	let name = path
		.file_name()
		.ok_or_else(|| "Некорректное имя файла".to_string())?
		.to_owned();
	path = tokio::fs::canonicalize(parent)
		.await
		.map_err(|_| "Каталог экспорта недоступен".to_string())?
		.join(name);
	state
		.file_grants
		.lock()
		.await
		.insert(path.clone(), FileGrant::Export);
	Ok(Some(path.to_string_lossy().into_owned()))
}
async fn grant(state: &tauri::State<'_, AppState>, path: &str, kind: FileGrant) -> Result<PathBuf> {
	let path = PathBuf::from(path);
	if state.file_grants.lock().await.get(&path).copied() != Some(kind) {
		return Err("Сначала выберите файл через диалог лаунчера".into());
	}
	Ok(path)
}
#[tauri::command]
pub async fn core_import_pack(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	path: String,
	name: String,
) -> Result<Instance> {
	let e = engine(&window, &state).await?;
	let path = grant(&state, &path, FileGrant::Import).await?;
	let op = e.begin("import_pack", None);
	let _ = app.emit_to("main", "core-progress", op.snapshot());
	let result = e.import_pack(&path, &name, &op).await;
	op.finish(
		&result
			.as_ref()
			.map(|_| ())
			.map_err(|e| ncreate_launcher_core::Error::Invalid(e.to_string())),
	);
	let _ = app.emit_to("main", "core-progress", op.snapshot());
	result.map_err(core_error)
}
#[tauri::command]
pub async fn core_export_pack(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	path: String,
) -> Result<()> {
	let e = engine(&window, &state).await?;
	let path = grant(&state, &path, FileGrant::Export).await?;
	e.export_pack(&instance_id, &path).await.map_err(core_error)
}
#[tauri::command]
pub async fn pick_instance_icon(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
) -> Result<Option<Instance>> {
	let e = engine(&window, &state).await?;
	let Some(path) = pick(app, false, true).await? else {
		return Ok(None);
	};
	e.set_instance_icon(&instance_id, &path)
		.await
		.map(Some)
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_editions(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	channel: String,
) -> Result<Vec<EditionAvailability>> {
	engine(&window, &state)
		.await?
		.edition_availability(&channel)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_install_edition(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	edition_id: String,
	channel: String,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	Ok(job(
		app,
		e,
		"install_edition",
		None,
		move |e, op| async move {
			e.install_edition(&edition_id, &channel, &op)
				.await
				.map(|_| ())
		},
	))
}
#[tauri::command]
pub async fn core_check_edition_update(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	channel: String,
) -> Result<UpdatePlan> {
	engine(&window, &state)
		.await?
		.check_edition_update(&instance_id, &channel)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_apply_edition_update(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	channel: String,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	let id = instance_id.clone();
	Ok(job(
		app,
		e,
		"update_edition",
		Some(&id),
		move |e, op| async move {
			e.apply_edition_update(&instance_id, &channel, &op)
				.await
				.map(|_| ())
		},
	))
}
#[tauri::command]
pub async fn core_resume_edition_install(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	channel: String,
) -> Result<String> {
	let e = engine(&window, &state).await?;
	let id = instance_id.clone();
	Ok(job(
		app,
		e,
		"resume_edition",
		Some(&id),
		move |e, op| async move {
			e.resume_edition_install(&instance_id, &channel, &op)
				.await
				.map(|_| ())
		},
	))
}

#[tauri::command]
pub async fn core_pick_java(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	instance_id: Option<String>,
) -> Result<Option<ncreate_launcher_core::JavaRuntime>> {
	let e = engine(&window, &state).await?;
	let path = tauri::async_runtime::spawn_blocking(move || {
		let dialog = app
			.dialog()
			.file()
			.set_title("Выберите bin/java или bin/java.exe");
		#[cfg(windows)]
		let dialog = dialog.add_filter("Java executable", &["exe"]);
		dialog.blocking_pick_file().map(|file| file.into_path())
	})
	.await
	.map_err(|_| "Не удалось открыть выбор Java".to_string())?
	.transpose()
	.map_err(|_| "Выберите локальный executable Java".to_string())?;
	let Some(path) = path else {
		return Ok(None);
	};
	let path = path.to_string_lossy();
	e.validate_java(&path, instance_id.as_deref())
		.await
		.map(Some)
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_select_java(
	window: tauri::WebviewWindow,
	state: tauri::State<'_, AppState>,
	instance_id: String,
	path: String,
) -> Result<ncreate_launcher_core::JavaRuntime> {
	engine(&window, &state)
		.await?
		.select_java(&instance_id, &path)
		.await
		.map_err(core_error)
}
#[tauri::command]
pub async fn core_java_download(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	major: u32,
) -> Result<()> {
	local(&window)?;
	if !(8..=99).contains(&major) {
		return Err("Некорректная версия Java".into());
	}
	let os = if cfg!(windows) { "windows" } else { "linux" };
	let arch = if cfg!(target_arch = "aarch64") {
		"aarch64"
	} else {
		"x64"
	};
	app.opener()
		.open_url(
			format!(
				"https://adoptium.net/temurin/releases/?version={major}&os={os}&arch={arch}&package=jdk"
			),
			None::<&str>,
		)
		.map_err(|_| "Не удалось открыть страницу Java".into())
}
