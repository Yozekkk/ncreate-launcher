//! Manual end-to-end network test of the published NCreate Server pack.
//! Ignored in CI because it downloads the full pack and Minecraft runtime.

#[tokio::test]
#[ignore = "downloads the published pack and full Minecraft runtime"]
async fn install_published_ncreate_server() {
	use ncreate_launcher_core::Engine;
	let temporary = tempfile::tempdir().expect("temporary launcher directory");
	let root = std::env::var_os("NCREATE_PACK_TEST_ROOT")
		.map(std::path::PathBuf::from)
		.unwrap_or_else(|| temporary.path().to_path_buf());
	let pool = sqlx::SqlitePool::connect("sqlite::memory:")
		.await
		.expect("temporary database");
	let engine = Engine::open(root, pool).await.expect("engine");
	let editions = engine
		.edition_availability("stable")
		.await
		.expect("manifest");
	let edition = &editions[0];
	assert!(
		edition.available,
		"published pack unavailable: {:?}",
		edition.error
	);
	let manifest = edition.manifest.as_ref().expect("published manifest");
	assert_eq!(manifest.version, "1.0.2");
	assert_eq!(manifest.minecraft, "1.21.1");
	assert_eq!(manifest.servers.len(), 1);
	assert_eq!(manifest.servers[0].address, "play.ncreate.online:25076");
	let operation = engine.begin("install_edition", None);
	let installed = engine
		.install_edition("ncreate-server", "stable", &operation)
		.await
		.expect("install published NCreate server pack");
	assert_eq!(installed.status, "ready");
	assert_eq!(installed.kind, "official");
	assert_eq!(installed.edition.as_deref(), Some("ncreate-server"));
	assert_eq!(
		engine.instances().await.expect("library state")[0].mod_count,
		147
	);
	let instance_path = std::path::Path::new(&installed.directory);
	let servers = tokio::fs::read(instance_path.join("servers.dat"))
		.await
		.expect("NCreate server list");
	assert!(
		servers
			.windows(b"play.ncreate.online:25076".len())
			.any(|window| { window == b"play.ncreate.online:25076" })
	);
	assert!(
		instance_path
			.join("resourcepacks/NCreate_Russian_Overrides.zip")
			.is_file()
	);
	assert!(instance_path.join("mods").is_dir());
	assert!(!instance_path.join("saves").exists());
	println!(
		"Installed published NCreate Server pack: {}",
		installed.directory
	);
}
