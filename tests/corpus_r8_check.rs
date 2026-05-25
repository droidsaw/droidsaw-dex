//! R8 corpus integration test for the inversion-pass track.
//!
//! Walks `tests/corpus/clean/r8-<ver>-release/<construct>/` and verifies:
//!
//! 1. Every `signature.toml` parses cleanly into an `R8SignatureSpec`.
//! 2. Every `verify.toml` parses cleanly into a `VerifySpec`.
//! 3. Every `.java` source compiles with the pinned `javac`.
//! 4. The compiled `.class` files feed through R8 with the construct's
//!    `rules.pro` to produce a transformed DEX.
//!
//! Skips with a clear diagnostic when `javac` / R8 / `android.jar` are
//! unavailable on the host. CI provides them; contributors without an
//! Android SDK get the skip path. The walker is the foundation for
//! Wave 2 sub-streams (`dex-r8-signatures`,
//! `dex-r8-inner-class-promotion-back`) which add recognisers that
//! consume the produced DEXes + their `verify.toml` assertions.
//!
//! Generated `.dex` artifacts land in `target/corpus-r8-out/`; `target/`
//! is workspace-gitignored so no per-stream `.gitignore` change is
//! required.

use std::path::{Path, PathBuf};
use std::process::Command;

use droidsaw_fixture_harness::{check_warnings_strict, skipped_outcome};

// ── schema ────────────────────────────────────────────────────────────────

/// Per-construct R8 signature loaded from `signature.toml`. Mirrors the
/// javac/kotlinc `SignatureSpec` (in `corpus_check.rs`) but with R8 as
/// the toolchain. Field set is intentionally identical so a future
/// unification can share the parser if needed.
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct R8SignatureSpec {
    toolchain: String,
    version: String,
    construct: String,
    discriminant_kind: String,
    outer_form: String,
    arm_pattern: String,
    #[serde(default)]
    inner_form: Option<String>,
    recovers: String,
    version_pin: R8VersionPin,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct R8VersionPin {
    #[serde(default)]
    javac: Option<String>,
    #[serde(default)]
    d8: Option<String>,
}

impl R8SignatureSpec {
    fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

/// Per-construct structural-assertion stub loaded from `verify.toml`.
/// Concrete Wave 2 recognisers will populate the `[expected]` table
/// with positive assertions; this stream only enforces schema parses.
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct VerifySpec {
    construct: String,
    expected_transform: String,
    #[serde(default)]
    expected: toml::Table,
}

impl VerifySpec {
    fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

// ── toolchain discovery ───────────────────────────────────────────────────

struct R8Tools {
    javac: PathBuf,
    r8_jar: PathBuf,
    android_jar: PathBuf,
}

impl R8Tools {
    fn resolve() -> Option<Self> {
        Some(R8Tools {
            javac: resolve_javac()?,
            r8_jar: resolve_r8_jar()?,
            android_jar: resolve_android_jar()?,
        })
    }
}

fn resolve_javac() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = Path::new(&home).join("bin").join("javac");
        if p.exists() {
            return Some(p);
        }
    }
    let out = Command::new("which").arg("javac").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn resolve_r8_jar() -> Option<PathBuf> {
    // Prefer R8_JAR env var, then well-known cmdline-tools locations.
    if let Ok(p) = std::env::var("R8_JAR") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let candidates = [
        "/opt/homebrew/share/android-commandlinetools/cmdline-tools/latest/lib/r8.jar",
        "/usr/local/share/android-commandlinetools/cmdline-tools/latest/lib/r8.jar",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(android_home) = std::env::var("ANDROID_HOME") {
        let p = Path::new(&android_home)
            .join("cmdline-tools")
            .join("latest")
            .join("lib")
            .join("r8.jar");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn resolve_android_jar() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ANDROID_JAR") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(android_home) = std::env::var("ANDROID_HOME") {
        let platforms = Path::new(&android_home).join("platforms");
        if let Ok(read) = std::fs::read_dir(&platforms) {
            let mut versions: Vec<PathBuf> =
                read.filter_map(|e| e.ok().map(|e| e.path())).collect();
            // Sort by basename then take the newest.
            versions.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
            for v in versions.into_iter().rev() {
                let jar = v.join("android.jar");
                if jar.exists() {
                    return Some(jar);
                }
            }
        }
    }
    // Last-ditch: Mac default location.
    let mac_default = PathBuf::from(format!(
        "{}/Library/Android/sdk/platforms/android-35/android.jar",
        std::env::var("HOME").ok().unwrap_or_default(),
    ));
    if mac_default.exists() {
        return Some(mac_default);
    }
    None
}

// ── compile + R8 ──────────────────────────────────────────────────────────

#[derive(Debug)]
struct StageFailure {
    stage: &'static str,
    detail: String,
}

fn javac_compile(tools: &R8Tools, src: &Path, class_dir: &Path) -> Result<(), StageFailure> {
    let out = Command::new(&tools.javac)
        .arg("--release")
        .arg("21")
        .arg("-Xlint:none")
        .arg("-d")
        .arg(class_dir)
        .arg(src)
        .output()
        .map_err(|e| StageFailure {
            stage: "javac spawn",
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(StageFailure {
            stage: "javac",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

fn r8_transform(
    tools: &R8Tools,
    class_dir: &Path,
    rules: &Path,
    dex_dir: &Path,
) -> Result<(), StageFailure> {
    let entries = std::fs::read_dir(class_dir).map_err(|e| StageFailure {
        stage: "read class_dir",
        detail: e.to_string(),
    })?;
    let class_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .flat_map(|e| collect_class_files(e.path()))
        .collect();
    if class_files.is_empty() {
        return Err(StageFailure {
            stage: "R8",
            detail: "no .class files produced by javac".to_string(),
        });
    }
    std::fs::create_dir_all(dex_dir).map_err(|e| StageFailure {
        stage: "mkdir dex_dir",
        detail: e.to_string(),
    })?;
    // mapping.txt sits alongside the DEX. R8's `--pg-map-output`
    // writes the Proguard/R8 mapping format (renames + inline
    // attribution when -keepattributes SourceFile,LineNumberTable is
    // active). Consumers (`r8_inversion::recognise_method_inlined`
    // oracle path, future `--mapping-file` CLI flag on top binary)
    // read it; tests that don't need it ignore the file.
    let mapping_path = dex_dir.join("mapping.txt");
    let out = Command::new("java")
        .arg("-cp")
        .arg(&tools.r8_jar)
        .arg("com.android.tools.r8.R8")
        .arg("--release")
        .arg("--output")
        .arg(dex_dir)
        .arg("--lib")
        .arg(&tools.android_jar)
        .arg("--pg-conf")
        .arg(rules)
        .arg("--pg-map-output")
        .arg(&mapping_path)
        .arg("--min-api")
        .arg("24")
        .args(&class_files)
        .output()
        .map_err(|e| StageFailure {
            stage: "R8 spawn",
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(StageFailure {
            stage: "R8",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

fn collect_class_files(p: PathBuf) -> Vec<PathBuf> {
    if p.is_file() {
        if p.extension().and_then(|e| e.to_str()) == Some("class") {
            return vec![p];
        }
        return vec![];
    }
    if !p.is_dir() {
        return vec![];
    }
    let Ok(read) = std::fs::read_dir(&p) else {
        return vec![];
    };
    read.filter_map(|e| e.ok())
        .flat_map(|e| collect_class_files(e.path()))
        .collect()
}

// ── corpus walker ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct R8CorpusEntry {
    toolchain_dir: String,
    construct: String,
    /// Path to `<construct>/` (contains source.java, rules.pro, etc).
    dir: PathBuf,
    /// First .java source under `<construct>/`.
    java_src: PathBuf,
    /// `<construct>/rules.pro`.
    rules: PathBuf,
    /// `<construct>/signature.toml`.
    signature: PathBuf,
    /// `<construct>/verify.toml`.
    verify: PathBuf,
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
}

fn discover_r8_corpus() -> std::io::Result<Vec<R8CorpusEntry>> {
    let mut entries = Vec::new();
    let clean = corpus_root().join("clean");
    if !clean.is_dir() {
        return Ok(entries);
    }
    for tc_entry in std::fs::read_dir(&clean)? {
        let tc = tc_entry?;
        if !tc.file_type()?.is_dir() {
            continue;
        }
        let name = tc.file_name().to_string_lossy().into_owned();
        if !name.starts_with("r8-") {
            continue;
        }
        for ct_entry in std::fs::read_dir(tc.path())? {
            let ct = ct_entry?;
            if !ct.file_type()?.is_dir() {
                continue;
            }
            let construct = ct.file_name().to_string_lossy().into_owned();
            let dir = ct.path();
            // Pick the source .java — that is, any .java NOT named
            // `expected-after-inversion.java` (which is documentation of
            // the Wave 2A recogniser's expected output, not a compile
            // input).
            let java_src = match find_source_java(&dir) {
                Some(p) => p,
                None => continue,
            };
            let rules = dir.join("rules.pro");
            let signature = dir.join("signature.toml");
            let verify = dir.join("verify.toml");
            entries.push(R8CorpusEntry {
                toolchain_dir: name.clone(),
                construct,
                dir,
                java_src,
                rules,
                signature,
                verify,
            });
        }
    }
    Ok(entries)
}

fn find_source_java(dir: &Path) -> Option<PathBuf> {
    let read = std::fs::read_dir(dir).ok()?;
    let mut candidates: Vec<PathBuf> = read
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("java"))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n != "expected-after-inversion.java")
                .unwrap_or(false)
        })
        .collect();
    // Sort for determinism — multiple source files per construct are
    // not the intended pattern, but if it ever happens, pick the
    // alphabetically-first.
    candidates.sort();
    candidates.into_iter().next()
}

// ── tests ─────────────────────────────────────────────────────────────────

#[test]
fn r8_signature_toml_parses_minimal() {
    let s = r#"
toolchain = "r8"
version = "9.0"
construct = "block_outlining"
discriminant_kind = "MethodBody"
outer_form = "single invoke-static"
arm_pattern = "trampoline"
recovers = "Stmt::OutlinedBlock { ... }"

[version_pin]
javac = "21"
"#;
    let spec = R8SignatureSpec::parse(s).expect("valid R8 signature.toml parses");
    assert_eq!(spec.toolchain, "r8");
    assert_eq!(spec.version, "9.0");
    assert_eq!(spec.construct, "block_outlining");
}

#[test]
fn verify_toml_parses_minimal() {
    let s = r#"
construct = "block_outlining"
expected_transform = "BlockOutlined"
"#;
    let spec = VerifySpec::parse(s).expect("minimal verify.toml parses");
    assert_eq!(spec.construct, "block_outlining");
    assert_eq!(spec.expected_transform, "BlockOutlined");
}

#[test]
fn verify_toml_with_expected_table_parses() {
    let s = r#"
construct = "method_inlining"
expected_transform = "MethodInlined"

[expected]
helper_method_eliminated = true
inlined_branch_count_min = 2
"#;
    let spec = VerifySpec::parse(s).expect("verify.toml with [expected] table parses");
    assert_eq!(spec.expected.get("helper_method_eliminated"), Some(&toml::Value::Boolean(true)));
}

#[test]
fn r8_corpus_signature_and_verify_files_parse() {
    let entries = discover_r8_corpus().expect("discover_r8_corpus");
    if entries.is_empty() {
        eprintln!("SKIP: no R8 corpus entries under tests/corpus/clean/r8-*-release/");
        return;
    }
    for entry in &entries {
        let sig_raw = std::fs::read_to_string(&entry.signature).unwrap_or_else(|e| {
            panic!(
                "read signature.toml at {}: {}",
                entry.signature.display(),
                e
            )
        });
        let _ = R8SignatureSpec::parse(&sig_raw).unwrap_or_else(|e| {
            panic!(
                "signature.toml parse failed at {}: {}",
                entry.signature.display(),
                e
            )
        });
        let ver_raw = std::fs::read_to_string(&entry.verify).unwrap_or_else(|e| {
            panic!("read verify.toml at {}: {}", entry.verify.display(), e)
        });
        let _ = VerifySpec::parse(&ver_raw).unwrap_or_else(|e| {
            panic!(
                "verify.toml parse failed at {}: {}",
                entry.verify.display(),
                e
            )
        });
        eprintln!(
            "OK schema: {}/{}",
            entry.toolchain_dir, entry.construct
        );
    }
    assert!(
        entries.len() >= 4,
        "expected ≥4 R8 corpus entries; found {}",
        entries.len(),
    );
}

#[test]
fn r8_corpus_pipeline_runs() {
    let entries = discover_r8_corpus().expect("discover_r8_corpus");
    if entries.is_empty() {
        eprintln!("SKIP: no R8 corpus entries");
        return;
    }
    let tools = match R8Tools::resolve() {
        Some(t) => t,
        None => {
            let outcome = skipped_outcome(
                "r8_corpus_pipeline_runs",
                "javac+R8+android.jar",
                "R8 toolchain unavailable. Set R8_JAR (full path to r8.jar), \
                 ANDROID_JAR (full path to android.jar), and JAVA_HOME / `javac` \
                 on PATH. CI provides the Android cmdline-tools + android-35.",
            );
            for w in &outcome.warnings {
                eprintln!("SKIP: {w:?}");
            }
            check_warnings_strict(&[outcome]).expect("strict-warnings gate");
            return;
        }
    };
    let out_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("corpus-r8-out");
    std::fs::create_dir_all(&out_root).expect("mkdir target/corpus-r8-out");

    let mut transformed = 0usize;
    let mut failures = Vec::<String>::new();
    for entry in &entries {
        let entry_out = out_root.join(&entry.toolchain_dir).join(&entry.construct);
        let class_dir = entry_out.join("classes");
        let dex_dir = entry_out.join("dex");
        let _ = std::fs::remove_dir_all(&class_dir);
        let _ = std::fs::remove_dir_all(&dex_dir);
        std::fs::create_dir_all(&class_dir).expect("mkdir class_dir");

        if let Err(e) = javac_compile(&tools, &entry.java_src, &class_dir) {
            failures.push(format!(
                "{} ({}/{}): javac/{}: {}",
                entry.java_src.display(),
                entry.toolchain_dir,
                entry.construct,
                e.stage,
                e.detail,
            ));
            continue;
        }
        if !entry.rules.exists() {
            failures.push(format!(
                "{}/{}: missing rules.pro at {}",
                entry.toolchain_dir,
                entry.construct,
                entry.rules.display(),
            ));
            continue;
        }
        if let Err(e) = r8_transform(&tools, &class_dir, &entry.rules, &dex_dir) {
            failures.push(format!(
                "{} ({}/{}): R8/{}: {}",
                entry.java_src.display(),
                entry.toolchain_dir,
                entry.construct,
                e.stage,
                e.detail,
            ));
            continue;
        }
        // R8 produces classes.dex (or classes2.dex etc) in dex_dir.
        let produced = std::fs::read_dir(&dex_dir)
            .ok()
            .map(|r| {
                r.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path().extension().and_then(|s| s.to_str()) == Some("dex")
                    })
                    .count()
            })
            .unwrap_or(0);
        if produced == 0 {
            failures.push(format!(
                "{}/{}: R8 succeeded but no .dex produced under {}",
                entry.toolchain_dir,
                entry.construct,
                dex_dir.display(),
            ));
            continue;
        }
        // Force a read of `entry.dir` to silence dead-field lint if it's
        // ever wired in; in practice the linter doesn't fire here, but
        // the field documents intent.
        let _ = &entry.dir;
        transformed += 1;
    }
    if !failures.is_empty() {
        panic!(
            "{} of {} R8 corpus entries failed:\n  - {}",
            failures.len(),
            entries.len(),
            failures.join("\n  - "),
        );
    }
    assert!(
        transformed >= 4,
        "expected ≥4 R8 corpus entries to transform; got {transformed}",
    );
    eprintln!(
        "r8_corpus_pipeline_runs: {transformed}/{} entries javac→R8 cleanly",
        entries.len(),
    );
}
