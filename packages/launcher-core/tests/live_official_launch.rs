//! Manual launch smoke test for an already installed NCreate Server pack.
//! Set NCREATE_PACK_TEST_ROOT to an isolated root populated by live_official_pack.

#[tokio::test]
#[ignore = "launches the full published modpack in an isolated test root"]
async fn launch_published_pack_offline() {
	use ncreate_launcher_core::{Engine, Instance, LaunchIdentity, Loader};
	let root = std::path::PathBuf::from(
		std::env::var_os("NCREATE_PACK_TEST_ROOT")
			.expect("set NCREATE_PACK_TEST_ROOT to the isolated installed pack"),
	);
	let instances_root = root.join("instances");
	let mut entries = tokio::fs::read_dir(&instances_root)
		.await
		.expect("installed instances directory");
	let entry = loop {
		let entry = entries
			.next_entry()
			.await
			.expect("read instances")
			.expect("installed instance with Minecraft runtime");
		if entry.path().join(".ncreate-runtime/version.json").is_file() {
			break entry;
		}
	};
	let id = entry.file_name().to_string_lossy().into_owned();
	let directory = entry.path();
	assert!(directory.join("mods").is_dir());
	let pool = sqlx::SqlitePool::connect("sqlite::memory:")
		.await
		.expect("temporary database");
	let engine = Engine::open(root, pool.clone()).await.expect("engine");
	let instance = Instance {
		id: id.clone(),
		name: "NCreate Server launch smoke".into(),
		game_version: "1.21.1".into(),
		loader: Loader::Neoforge,
		loader_version: Some("21.1.250".into()),
		kind: "official".into(),
		edition: Some("ncreate-server".into()),
		manifest_version: Some("1.0.1".into()),
		status: "ready".into(),
		memory_mb: 5120,
		java_path: None,
		directory: directory.to_string_lossy().into_owned(),
		icon: None,
		mod_count: 147,
		last_played: None,
	};
	sqlx::query("INSERT INTO launcher_instances(id, data) VALUES (?, ?)")
		.bind(&id)
		.bind(serde_json::to_string(&instance).expect("serialize instance"))
		.execute(&pool)
		.await
		.expect("restore instance metadata for smoke test");
	let running = engine
		.launch(
			&id,
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
		.expect("start published pack");
	println!("Published pack game PID: {}", running.pid);
	tokio::time::sleep(std::time::Duration::from_secs(45)).await;
	let state = engine.instance(&id).await.expect("poll game process");
	if state.status != "running" {
		let log = tokio::fs::read_to_string(&running.log_path)
			.await
			.unwrap_or_default();
		println!(
			"Game log tail: {}",
			log.chars()
				.rev()
				.take(4000)
				.collect::<String>()
				.chars()
				.rev()
				.collect::<String>()
		);
	}
	assert_eq!(state.status, "running", "game exited before smoke interval");
	engine.stop(&id).await.expect("stop game");
}
