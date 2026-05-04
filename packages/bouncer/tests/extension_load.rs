use std::env;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::path::PathBuf;
use std::process::Command;

use rusqlite::Connection;

#[test]
fn loadable_extension_registers_all_bouncer_functions() -> rusqlite::Result<()> {
    let artifact = build_extension_artifact(BuildProfile::Debug);
    let conn = Connection::open_in_memory()?;
    load_extension(&conn, &artifact)?;
    assert_full_smoke_path(&conn)
}

#[test]
fn release_extension_artifact_loads_full_smoke_path() -> rusqlite::Result<()> {
    let artifact = build_extension_artifact(BuildProfile::Release);
    let conn = Connection::open_in_memory()?;
    load_extension(&conn, &artifact)?;
    assert_full_smoke_path(&conn)
}

fn assert_full_smoke_path(conn: &Connection) -> rusqlite::Result<()> {
    let bootstrapped: i64 = conn.query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))?;
    assert_eq!(bootstrapped, 1);

    let claimed: Option<i64> = conn.query_row(
        "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
        ("scheduler", "worker-a", 5_000_i64, 1_000_i64),
        |row| row.get(0),
    )?;
    assert_eq!(claimed, Some(1));

    let busy: Option<i64> = conn.query_row(
        "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
        ("scheduler", "worker-b", 5_000_i64, 1_001_i64),
        |row| row.get(0),
    )?;
    assert_eq!(busy, None);

    let owner: Option<String> = conn.query_row(
        "SELECT bouncer_owner(?1, ?2)",
        ("scheduler", 1_002_i64),
        |row| row.get(0),
    )?;
    assert_eq!(owner.as_deref(), Some("worker-a"));

    let renewed: Option<i64> = conn.query_row(
        "SELECT bouncer_renew(?1, ?2, ?3, ?4)",
        ("scheduler", "worker-a", 8_000_i64, 1_003_i64),
        |row| row.get(0),
    )?;
    assert_eq!(renewed, Some(1));

    let token: Option<i64> =
        conn.query_row("SELECT bouncer_token(?1)", ("scheduler",), |row| row.get(0))?;
    assert_eq!(token, Some(1));

    let released: i64 = conn.query_row(
        "SELECT bouncer_release(?1, ?2, ?3)",
        ("scheduler", "worker-a", 1_004_i64),
        |row| row.get(0),
    )?;
    assert_eq!(released, 1);

    let owner_after_release: Option<String> = conn.query_row(
        "SELECT bouncer_owner(?1, ?2)",
        ("scheduler", 1_005_i64),
        |row| row.get(0),
    )?;
    assert_eq!(owner_after_release, None);

    Ok(())
}

#[derive(Clone, Copy)]
enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn as_str(self) -> &'static str {
        match self {
            BuildProfile::Debug => "debug",
            BuildProfile::Release => "release",
        }
    }
}

fn build_extension_artifact(profile: BuildProfile) -> PathBuf {
    let repo_root = repo_root();
    let mut command = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    command
        .arg("build")
        .arg("-p")
        .arg("bouncer-extension");
    if matches!(profile, BuildProfile::Release) {
        command.arg("--release");
    }
    let status = command
        .current_dir(&repo_root)
        .status()
        .expect("run cargo build for bouncer-extension");
    assert!(
        status.success(),
        "cargo build -p bouncer-extension{} failed",
        if matches!(profile, BuildProfile::Release) {
            " --release"
        } else {
            ""
        }
    );

    let artifact = target_dir(&repo_root)
        .join(profile.as_str())
        .join(format!("{DLL_PREFIX}bouncer_ext{DLL_SUFFIX}"));
    assert!(
        artifact.exists(),
        "missing bouncer extension artifact at {}",
        artifact.display()
    );
    artifact
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|packages| packages.parent())
        .expect("packages/bouncer lives under repo root")
        .to_path_buf()
}

fn target_dir(repo_root: &std::path::Path) -> PathBuf {
    match env::var_os("CARGO_TARGET_DIR") {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                repo_root.join(path)
            }
        }
        None => repo_root.join("target"),
    }
}

fn load_extension(conn: &Connection, artifact: &std::path::Path) -> rusqlite::Result<()> {
    unsafe {
        conn.load_extension_enable()?;
        let result = conn.load_extension(artifact, None::<&str>);
        conn.load_extension_disable()?;
        result
    }
}
