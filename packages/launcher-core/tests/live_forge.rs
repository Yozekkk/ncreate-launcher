//! Manual network smoke test for the upstream Forge metadata and processor pipeline.
//! Ignored in CI because it downloads Minecraft and Forge assets.

#[tokio::test]
#[ignore = "downloads the full Minecraft 1.21.1 Forge runtime"]
async fn install_forge_1_21_1() {
	use ncreate_launcher_core::{CreateInstance, Engine, LaunchIdentity, Loader};
	let temporary = tempfile::tempdir().expect("temporary launcher directory");
	let root = std::env::var_os("NCREATE_FORGE_TEST_ROOT")
		.map(std::path::PathBuf::from)
		.unwrap_or_else(|| temporary.path().to_path_buf());
	let pool = sqlx::SqlitePool::connect("sqlite::memory:")
		.await
		.expect("temporary database");
	let engine = Engine::open(root, pool).await.expect("engine");
	let instance = engine
		.create_instance(CreateInstance {
			name: "Forge integration test".into(),
			game_version: "1.21.1".into(),
			loader: Loader::Forge,
			loader_version: Some("52.1.14".into()),
			memory_mb: 4096,
			java_path: None,
		})
		.await
		.expect("create instance");
	let op = engine.begin("install_game", Some(&instance.id));
	let installed = engine
		.install_game(&instance.id, &op)
		.await
		.expect("install Forge game runtime");
	assert_eq!(installed.status, "ready");
	if std::env::var_os("NCREATE_FORGE_TEST_LAUNCH").is_some() {
		let game = engine
			.launch(
				&instance.id,
				LaunchIdentity {
					nickname: "NCreateQA".into(),
					uuid: "88611cf2-09a6-3ea9-b10a-62faecf8157a".into(),
					access_token: "0".into(),
					user_type: "legacy".into(),
					xuid: None,
					authlib_injector: None,
				},
			)
			.await
			.expect("start Forge game");
		println!("Forge game PID: {}", game.pid);
		tokio::time::sleep(std::time::Duration::from_secs(20)).await;
		assert_eq!(
			engine.instance(&instance.id).await.unwrap().status,
			"running"
		);
		engine.stop(&instance.id).await.expect("stop Forge game");
	}
}
