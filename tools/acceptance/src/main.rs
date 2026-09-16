use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(
    name = "pomme-acceptance",
    about = "Run Pomme public-path acceptance scenarios"
)]
struct Cli {
    #[arg(long, default_value = "acceptance/scenarios.json")]
    manifest: PathBuf,

    /// Run only the selected scenario. Repeat to select more than one.
    #[arg(long)]
    scenario: Vec<String>,

    #[arg(long, default_value = "target/acceptance")]
    artifacts: PathBuf,

    /// Retain generated temporary game/world directories after the run.
    #[arg(long)]
    keep_temp: bool,

    /// Parse and validate the manifest without starting any process.
    #[arg(long)]
    list: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    id: String,
    description: String,
    protocol: String,
    server_fixture: String,
    fixture_hash: String,
    account_mode: String,
    timeout_seconds: u64,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    expected_output: Vec<String>,
    #[serde(default)]
    expected_files: Vec<ExpectedFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFile {
    path: String,
    #[serde(default)]
    kind: FileKind,
    #[serde(default)]
    min_bytes: u64,
    #[serde(default)]
    retain_as: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileKind {
    #[default]
    Any,
    Json,
    Png,
}

#[derive(Debug, Serialize)]
struct ScenarioResult {
    schema: u32,
    id: String,
    protocol: String,
    server_fixture: String,
    fixture_hash: String,
    account_mode: String,
    duration_ms: u128,
    exit_code: Option<i32>,
    timed_out: bool,
    passed: bool,
    checkpoints: Vec<String>,
    retained_artifacts: Vec<String>,
    failures: Vec<String>,
}

#[derive(Error, Debug)]
enum RunnerError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid manifest {path}: {source}")]
    Manifest {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("manifest validation failed: {0}")]
    Validation(String),

    #[error("failed to create acceptance directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
}

struct Variables {
    repo_root: PathBuf,
    run_dir: PathBuf,
    temp_dir: PathBuf,
    git_head: String,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("acceptance runner failed: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), RunnerError> {
    let repo_root = std::env::current_dir().map_err(|source| RunnerError::Read {
        path: PathBuf::from("."),
        source,
    })?;
    let manifest_path = absolute(&repo_root, &cli.manifest);
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|source| RunnerError::Read {
        path: manifest_path.clone(),
        source,
    })?;
    let manifest: Manifest =
        serde_json::from_str(&manifest_text).map_err(|source| RunnerError::Manifest {
            path: manifest_path,
            source,
        })?;
    validate_manifest(&manifest)?;

    let selected: Vec<&Scenario> = if cli.scenario.is_empty() {
        manifest.scenarios.iter().collect()
    } else {
        let requested: HashSet<&str> = cli.scenario.iter().map(String::as_str).collect();
        let selected: Vec<_> = manifest
            .scenarios
            .iter()
            .filter(|scenario| requested.contains(scenario.id.as_str()))
            .collect();
        if selected.len() != requested.len() {
            let found: HashSet<&str> = selected
                .iter()
                .map(|scenario| scenario.id.as_str())
                .collect();
            let missing = requested
                .difference(&found)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RunnerError::Validation(format!(
                "unknown selected scenario(s): {missing}"
            )));
        }
        selected
    };

    if cli.list {
        for scenario in selected {
            println!(
                "{}\tprotocol={}\tfixture={}\t{}",
                scenario.id, scenario.protocol, scenario.server_fixture, scenario.description
            );
        }
        return Ok(());
    }

    let run_id = format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        std::process::id()
    );
    let artifact_root = absolute(&repo_root, &cli.artifacts).join(run_id);
    create_dir(&artifact_root)?;
    let git_head = git_head(&repo_root);
    let mut failures = 0usize;

    for scenario in selected {
        let scenario_dir = artifact_root.join(&scenario.id);
        create_dir(&scenario_dir)?;
        let temp_dir = std::env::temp_dir().join(format!(
            "pomme-acceptance-{}-{}",
            scenario.id,
            std::process::id()
        ));
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        create_dir(&temp_dir)?;
        let variables = Variables {
            repo_root: repo_root.clone(),
            run_dir: scenario_dir.clone(),
            temp_dir: temp_dir.clone(),
            git_head: git_head.clone(),
        };

        println!("running {}: {}", scenario.id, scenario.description);
        let result = run_scenario(scenario, &variables);
        let result_path = scenario_dir.join("result.json");
        fs::write(
            &result_path,
            serde_json::to_vec_pretty(&result).expect("result serialization cannot fail"),
        )
        .map_err(|source| RunnerError::Read {
            path: result_path,
            source,
        })?;
        if result.passed {
            println!("PASS {} ({} ms)", scenario.id, result.duration_ms);
        } else {
            failures += 1;
            eprintln!("FAIL {}: {}", scenario.id, result.failures.join("; "));
        }
        if !cli.keep_temp {
            let _ = fs::remove_dir_all(temp_dir);
        }
    }

    println!("acceptance artifacts: {}", artifact_root.display());
    if failures == 0 {
        Ok(())
    } else {
        Err(RunnerError::Validation(format!(
            "{failures} scenario(s) failed"
        )))
    }
}

fn validate_manifest(manifest: &Manifest) -> Result<(), RunnerError> {
    if manifest.schema != 1 {
        return Err(RunnerError::Validation(format!(
            "unsupported manifest schema {}",
            manifest.schema
        )));
    }
    if manifest.scenarios.is_empty() {
        return Err(RunnerError::Validation("manifest has no scenarios".into()));
    }
    let mut ids = HashSet::new();
    for scenario in &manifest.scenarios {
        if scenario.id.is_empty()
            || !scenario
                .id
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            return Err(RunnerError::Validation(format!(
                "invalid scenario id {:?}",
                scenario.id
            )));
        }
        if !ids.insert(&scenario.id) {
            return Err(RunnerError::Validation(format!(
                "duplicate scenario id {}",
                scenario.id
            )));
        }
        if scenario.timeout_seconds == 0 {
            return Err(RunnerError::Validation(format!(
                "scenario {} has a zero timeout",
                scenario.id
            )));
        }
        if scenario.fixture_hash.is_empty() || scenario.account_mode.is_empty() {
            return Err(RunnerError::Validation(format!(
                "scenario {} is missing fixture/account metadata",
                scenario.id
            )));
        }
    }
    Ok(())
}

fn run_scenario(scenario: &Scenario, variables: &Variables) -> ScenarioResult {
    let stdout_path = variables.run_dir.join("stdout.log");
    let stderr_path = variables.run_dir.join("stderr.log");
    let mut failures = Vec::new();
    let started = Instant::now();

    let stdout = match File::create(&stdout_path) {
        Ok(file) => file,
        Err(error) => {
            return failed_result(
                scenario,
                variables,
                started,
                format!("failed to create {}: {error}", stdout_path.display()),
            );
        }
    };
    let stderr = match File::create(&stderr_path) {
        Ok(file) => file,
        Err(error) => {
            return failed_result(
                scenario,
                variables,
                started,
                format!("failed to create {}: {error}", stderr_path.display()),
            );
        }
    };

    let mut command = Command::new(substitute(&scenario.command, variables));
    command.current_dir(&variables.repo_root);
    command.args(
        scenario
            .args
            .iter()
            .map(|argument| substitute(argument, variables)),
    );
    command.stdout(Stdio::from(stdout));
    command.stderr(Stdio::from(stderr));
    for (key, value) in &scenario.env {
        command.env(key, substitute(value, variables));
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return failed_result(
                scenario,
                variables,
                started,
                format!("failed to start command: {error}"),
            );
        }
    };
    let timeout = Duration::from_secs(scenario.timeout_seconds);
    let (status, timed_out) = wait_bounded(&mut child, timeout);

    let mut combined = String::new();
    if let Ok(mut file) = File::open(&stdout_path) {
        let _ = file.read_to_string(&mut combined);
    }
    if let Ok(mut file) = File::open(&stderr_path) {
        let _ = file.read_to_string(&mut combined);
    }

    if timed_out {
        failures.push(format!(
            "timed out after {} seconds",
            scenario.timeout_seconds
        ));
    }
    match status.as_ref() {
        Some(status) if status.success() => {}
        Some(status) => failures.push(format_status(*status)),
        None if !timed_out => failures.push("process status unavailable".into()),
        None => {}
    }

    let mut checkpoints = Vec::new();
    for checkpoint in &scenario.expected_output {
        if combined.contains(checkpoint) {
            checkpoints.push(checkpoint.clone());
        } else {
            failures.push(format!("missing output checkpoint {checkpoint:?}"));
        }
    }

    let mut retained_artifacts = Vec::new();
    for expected in &scenario.expected_files {
        let pattern = PathBuf::from(substitute(&expected.path, variables));
        let paths = match matching_paths(&pattern) {
            Ok(paths) if !paths.is_empty() => paths,
            Ok(_) => {
                failures.push(format!(
                    "expected file pattern {} matched nothing",
                    pattern.display()
                ));
                continue;
            }
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let path = &paths[0];
        match validate_file(path, expected) {
            Ok(()) => {
                if let Some(name) = &expected.retain_as {
                    let destination = variables.run_dir.join(name);
                    if path.as_path() != destination.as_path() {
                        if let Some(parent) = destination.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        match fs::copy(&path, &destination) {
                            Ok(_) => retained_artifacts.push(name.clone()),
                            Err(error) => failures.push(format!(
                                "failed to retain {} as {}: {error}",
                                path.display(),
                                destination.display()
                            )),
                        }
                    } else {
                        retained_artifacts.push(name.clone());
                    }
                }
            }
            Err(error) => failures.push(error),
        }
    }

    ScenarioResult {
        schema: 1,
        id: scenario.id.clone(),
        protocol: scenario.protocol.clone(),
        server_fixture: scenario.server_fixture.clone(),
        fixture_hash: substitute(&scenario.fixture_hash, variables),
        account_mode: scenario.account_mode.clone(),
        duration_ms: started.elapsed().as_millis(),
        exit_code: status.and_then(|status| status.code()),
        timed_out,
        passed: failures.is_empty(),
        checkpoints,
        retained_artifacts,
        failures,
    }
}

fn failed_result(
    scenario: &Scenario,
    variables: &Variables,
    started: Instant,
    failure: String,
) -> ScenarioResult {
    ScenarioResult {
        schema: 1,
        id: scenario.id.clone(),
        protocol: scenario.protocol.clone(),
        server_fixture: scenario.server_fixture.clone(),
        fixture_hash: substitute(&scenario.fixture_hash, variables),
        account_mode: scenario.account_mode.clone(),
        duration_ms: started.elapsed().as_millis(),
        exit_code: None,
        timed_out: false,
        passed: false,
        checkpoints: Vec::new(),
        retained_artifacts: Vec::new(),
        failures: vec![failure],
    }
}

fn wait_bounded(child: &mut std::process::Child, timeout: Duration) -> (Option<ExitStatus>, bool) {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (Some(status), false),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                return (child.wait().ok(), true);
            }
            Err(_) => return (None, false),
        }
    }
}

fn validate_file(path: &Path, expected: &ExpectedFile) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("expected file {} is unavailable: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("expected path {} is not a file", path.display()));
    }
    if metadata.len() < expected.min_bytes {
        return Err(format!(
            "expected file {} has {} bytes, below minimum {}",
            path.display(),
            metadata.len(),
            expected.min_bytes
        ));
    }
    match expected.kind {
        FileKind::Any => Ok(()),
        FileKind::Json => {
            let file = File::open(path)
                .map_err(|error| format!("failed to open JSON {}: {error}", path.display()))?;
            serde_json::from_reader::<_, serde_json::Value>(file)
                .map(|_| ())
                .map_err(|error| format!("invalid JSON {}: {error}", path.display()))
        }
        FileKind::Png => {
            let mut signature = [0u8; 8];
            File::open(path)
                .and_then(|mut file| file.read_exact(&mut signature))
                .map_err(|error| format!("failed to read PNG {}: {error}", path.display()))?;
            if signature == *b"\x89PNG\r\n\x1a\n" {
                Ok(())
            } else {
                Err(format!("invalid PNG signature in {}", path.display()))
            }
        }
    }
}

/// Resolve an exact path or one `*` wildcard in the file-name component.
fn matching_paths(pattern: &Path) -> Result<Vec<PathBuf>, String> {
    let Some(file_name) = pattern.file_name().and_then(|name| name.to_str()) else {
        return Ok(vec![pattern.to_owned()]);
    };
    let Some((prefix, suffix)) = file_name.split_once('*') else {
        return Ok(vec![pattern.to_owned()]);
    };
    if suffix.contains('*') {
        return Err(format!(
            "expected file pattern {} contains more than one wildcard",
            pattern.display()
        ));
    }
    let parent = pattern.parent().unwrap_or_else(|| Path::new("."));
    let entries = fs::read_dir(parent).map_err(|error| {
        format!(
            "failed to read expected artifact directory {}: {error}",
            parent.display()
        )
    })?;
    let mut matches = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(suffix))
        })
        .collect::<Vec<_>>();
    matches.sort();
    Ok(matches)
}

fn substitute(input: &str, variables: &Variables) -> String {
    input
        .replace("${REPO_ROOT}", &variables.repo_root.to_string_lossy())
        .replace("${RUN_DIR}", &variables.run_dir.to_string_lossy())
        .replace("${TEMP_DIR}", &variables.temp_dir.to_string_lossy())
        .replace("${GIT_HEAD}", &variables.git_head)
}

fn format_status(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("process exited with code {code}"),
        None => "process terminated by signal".into(),
    }
}

fn git_head(repo_root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into())
}

fn absolute(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

fn create_dir(path: &Path) -> Result<(), RunnerError> {
    fs::create_dir_all(path).map_err(|source| RunnerError::CreateDir {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        ExpectedFile, FileKind, Manifest, Variables, matching_paths, substitute, validate_file,
        validate_manifest,
    };

    fn variables(root: &std::path::Path) -> Variables {
        Variables {
            repo_root: root.into(),
            run_dir: root.join("run"),
            temp_dir: root.join("temp"),
            git_head: "abc123".into(),
        }
    }

    #[test]
    fn substitutes_all_manifest_variables() {
        let root = std::path::Path::new("/repo");
        let vars = variables(root);
        let expected = format!(
            "{}:{}:{}:abc123",
            root.display(),
            root.join("run").display(),
            root.join("temp").display()
        );
        assert_eq!(
            substitute("${REPO_ROOT}:${RUN_DIR}:${TEMP_DIR}:${GIT_HEAD}", &vars),
            expected
        );
    }

    #[test]
    fn validates_json_and_png_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "pomme-acceptance-runner-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let json = root.join("trace.json");
        fs::write(&json, br#"{"ok":true}"#).unwrap();
        validate_file(
            &json,
            &ExpectedFile {
                path: String::new(),
                kind: FileKind::Json,
                min_bytes: 2,
                retain_as: None,
            },
        )
        .unwrap();

        let png = root.join("frame.png");
        fs::write(&png, b"\x89PNG\r\n\x1a\nrest").unwrap();
        validate_file(
            &png,
            &ExpectedFile {
                path: String::new(),
                kind: FileKind::Png,
                min_bytes: 8,
                retain_as: None,
            },
        )
        .unwrap();
        assert_eq!(matching_paths(&root.join("*.png")).unwrap(), vec![png]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_scenario_ids() {
        let manifest: Manifest = serde_json::from_str(
            r#"{
                "schema": 1,
                "scenarios": [
                    {
                        "id": "same",
                        "description": "one",
                        "protocol": "26.2",
                        "server_fixture": "fixture",
                        "fixture_hash": "hash",
                        "account_mode": "offline",
                        "timeout_seconds": 1,
                        "command": "true"
                    },
                    {
                        "id": "same",
                        "description": "two",
                        "protocol": "26.2",
                        "server_fixture": "fixture",
                        "fixture_hash": "hash",
                        "account_mode": "offline",
                        "timeout_seconds": 1,
                        "command": "true"
                    }
                ]
            }"#,
        )
        .unwrap();
        assert!(validate_manifest(&manifest).is_err());
    }
}
