//! Corpus-bootstrap integration test for the inversion-driven decompilation
//! track. Walks `tests/corpus/clean/<toolchain>-<version>/<construct>/` and
//! verifies:
//!
//! 1. Every `signature.toml` parses cleanly into a `SignatureSpec`.
//! 2. Every `.java` source compiles with the pinned `javac`.
//! 3. The compiled `.class` files convert to `.dex` via `d8`.
//!
//! Skips with a clear diagnostic when `javac` / `d8` are unavailable on the
//! host; CI provides both. The corpus harness is the foundation for stream
//! #3 (`dex-signature-engine`) which will add a `Signature` consumer that
//! reads the same `signature.toml` files.
//!
//! Generated `.dex` artifacts land in `target/corpus-out/`; `target/` is
//! workspace-gitignored so no per-stream `.gitignore` change is required.

use std::path::{Path, PathBuf};
use std::process::Command;

use droidsaw_fixture_harness::{check_warnings_strict, skipped_outcome};

// ── signature.toml schema ─────────────────────────────────────────────────

/// Per-construct fingerprint loaded from `signature.toml`. The fingerprint
/// is the source-of-truth spec for the lowering this construct's recognizer
/// (landed in stream #4) is expected to recover. This stream parses + validates
/// the schema; no recognizer consumes it yet.
///
/// `dead_code` is allowed crate-wide on this struct (and `VersionPin`) because
/// every field is load-bearing for schema validation: `toml::de` rejects the
/// document when a required field is missing or has the wrong type, even if
/// no test asserts on the parsed value. Stream #3 (`dex-signature-engine`)
/// will be the first reader; until then, the schema is enforced by the
/// deserializer, not by field-access patterns.
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct SignatureSpec {
    toolchain: String,
    version: String,
    construct: String,
    discriminant_kind: String,
    outer_form: String,
    arm_pattern: String,
    /// Optional — single-form constructs (e.g., `enhanced_for_*`) have no
    /// inner-form companion. The two-stage `switch_string` lowering does.
    #[serde(default)]
    inner_form: Option<String>,
    recovers: String,
    version_pin: VersionPin,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct VersionPin {
    #[serde(default)]
    javac: Option<String>,
    #[serde(default)]
    kotlinc: Option<String>,
    #[serde(default)]
    d8: Option<String>,
}

impl SignatureSpec {
    fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

// ── toolchain discovery ───────────────────────────────────────────────────

struct Tools {
    javac: PathBuf,
    /// Optional — only kotlinc-1.9 corpus entries need it. Resolution
    /// failure is non-fatal at the `Tools::resolve` level (per-entry
    /// dispatch surfaces a clear "kotlinc unavailable" failure when a
    /// kotlinc entry is encountered without the resolver succeeding).
    kotlinc: Option<PathBuf>,
    d8: PathBuf,
}

impl Tools {
    fn resolve() -> Option<Self> {
        Some(Tools {
            javac: resolve_javac()?,
            kotlinc: resolve_kotlinc(),
            d8: resolve_d8()?,
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

fn resolve_kotlinc() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("KOTLIN_HOME") {
        let p = Path::new(&home).join("bin").join("kotlinc");
        if p.exists() {
            return Some(p);
        }
    }
    let out = Command::new("which").arg("kotlinc").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn resolve_d8() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("ANDROID_HOME") {
        let bt = Path::new(&home).join("build-tools");
        if let Ok(read) = std::fs::read_dir(&bt) {
            let mut versions: Vec<PathBuf> =
                read.filter_map(|e| e.ok().map(|e| e.path())).collect();
            versions.sort();
            for v in versions.into_iter().rev() {
                let d8 = v.join("d8");
                if d8.exists() {
                    return Some(d8);
                }
            }
        }
    }
    let out = Command::new("which").arg("d8").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

// ── compile + d8 ──────────────────────────────────────────────────────────

#[derive(Debug)]
struct CompileFailure {
    stage: &'static str,
    detail: String,
}

fn javac_compile(tools: &Tools, src: &Path, class_dir: &Path) -> Result<(), CompileFailure> {
    let out = Command::new(&tools.javac)
        .arg("--release")
        .arg("21")
        .arg("-Xlint:none")
        .arg("-d")
        .arg(class_dir)
        .arg(src)
        .output()
        .map_err(|e| CompileFailure {
            stage: "javac spawn",
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(CompileFailure {
            stage: "javac",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

fn kotlinc_compile(tools: &Tools, src: &Path, class_dir: &Path) -> Result<(), CompileFailure> {
    let kotlinc = tools.kotlinc.as_ref().ok_or_else(|| CompileFailure {
        stage: "kotlinc unavailable",
        detail: "set KOTLIN_HOME (e.g. /path/to/kotlinc-1.9.22) or add kotlinc to PATH"
            .to_string(),
    })?;
    // Bundled kotlinx-coroutines jar lives next to kotlinc under the
    // distribution's lib/ directory. The `coroutine_suspend` corpus needs
    // it; other constructs do not, but adding it unconditionally is cheap
    // (kotlinc accepts the classpath even when no symbol is resolved
    // through it).
    let coroutines_jar = kotlinc
        .parent()
        .and_then(|bin| bin.parent())
        .map(|home| home.join("lib").join("kotlinx-coroutines-core-jvm.jar"))
        .filter(|p| p.exists());
    let mut cmd = Command::new(kotlinc);
    cmd.arg("-jvm-target").arg("21");
    if let Some(jar) = coroutines_jar.as_ref() {
        cmd.arg("-cp").arg(jar);
    }
    cmd.arg("-d").arg(class_dir).arg(src);
    let out = cmd.output().map_err(|e| CompileFailure {
        stage: "kotlinc spawn",
        detail: e.to_string(),
    })?;
    if !out.status.success() {
        return Err(CompileFailure {
            stage: "kotlinc",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

fn d8_convert(tools: &Tools, class_dir: &Path, dex_dir: &Path) -> Result<(), CompileFailure> {
    let entries = std::fs::read_dir(class_dir).map_err(|e| CompileFailure {
        stage: "read class_dir",
        detail: e.to_string(),
    })?;
    let class_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("class"))
        .collect();
    if class_files.is_empty() {
        return Err(CompileFailure {
            stage: "d8",
            detail: "no .class files produced by javac".to_string(),
        });
    }
    let out = Command::new(&tools.d8)
        .arg("--no-desugaring")
        .arg("--output")
        .arg(dex_dir)
        .args(&class_files)
        .output()
        .map_err(|e| CompileFailure {
            stage: "d8 spawn",
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(CompileFailure {
            stage: "d8",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

// ── corpus walker ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct CorpusEntry {
    /// e.g. `"javac-21"`.
    toolchain_dir: String,
    /// e.g. `"switch_string"`.
    construct: String,
    /// Path to the source file (`.java` or `.kt`).
    src: PathBuf,
    /// File stem, e.g. `"05arms"`.
    name: String,
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
}

fn discover_corpus() -> std::io::Result<Vec<CorpusEntry>> {
    let mut entries = Vec::new();
    let clean_dir = corpus_root().join("clean");
    if !clean_dir.is_dir() {
        return Ok(entries);
    }
    for tc_entry in std::fs::read_dir(&clean_dir)? {
        let tc_entry = tc_entry?;
        if !tc_entry.file_type()?.is_dir() {
            continue;
        }
        let toolchain_dir = tc_entry.file_name().to_string_lossy().into_owned();
        for ct_entry in std::fs::read_dir(tc_entry.path())? {
            let ct_entry = ct_entry?;
            if !ct_entry.file_type()?.is_dir() {
                continue;
            }
            let construct = ct_entry.file_name().to_string_lossy().into_owned();
            for src_entry in std::fs::read_dir(ct_entry.path())? {
                let src_entry = src_entry?;
                let path = src_entry.path();
                let ext = path.extension().and_then(|e| e.to_str());
                if ext == Some("java") || ext == Some("kt") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    entries.push(CorpusEntry {
                        toolchain_dir: toolchain_dir.clone(),
                        construct: construct.clone(),
                        src: path,
                        name,
                    });
                }
            }
        }
    }
    Ok(entries)
}

fn walkdir_signature_toml(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(read) = std::fs::read_dir(root) {
        for e in read.filter_map(|r| r.ok()) {
            let p = e.path();
            if p.is_dir() {
                out.extend(walkdir_signature_toml(&p));
            } else if p.file_name().and_then(|s| s.to_str()) == Some("signature.toml") {
                out.push(p);
            }
        }
    }
    out
}

// ── tests ─────────────────────────────────────────────────────────────────

#[test]
fn signature_toml_parses_minimal() {
    let toml_str = r#"
toolchain = "javac"
version = "21"
construct = "switch_string"
discriminant_kind = "String"
outer_form = "tableswitch on hashCode"
arm_pattern = "equals + tag-store"
recovers = "Stmt::MultiArm { ... }"

[version_pin]
javac = "21"
"#;
    let spec = SignatureSpec::parse(toml_str).expect("valid signature.toml parses");
    assert_eq!(spec.toolchain, "javac");
    assert_eq!(spec.version, "21");
    assert_eq!(spec.construct, "switch_string");
    assert_eq!(spec.discriminant_kind, "String");
    assert_eq!(spec.version_pin.javac.as_deref(), Some("21"));
    assert!(spec.inner_form.is_none());
}

#[test]
fn signature_toml_inner_form_optional() {
    let toml_str = r#"
toolchain = "javac"
version = "21"
construct = "enhanced_for_iterable"
discriminant_kind = "Iterable"
outer_form = "while-loop on Iterator.hasNext"
arm_pattern = "Iterator.next"
recovers = "Stmt::ForEach { ... }"

[version_pin]
javac = "21"
"#;
    let spec = SignatureSpec::parse(toml_str).expect("inner_form omitted is valid");
    assert_eq!(spec.inner_form, None);
}

#[test]
fn signature_toml_two_stage_with_inner_form() {
    let toml_str = r#"
toolchain = "javac"
version = "21"
construct = "switch_string"
discriminant_kind = "String"
outer_form = "tableswitch on hashCode"
arm_pattern = "equals + tag-store"
inner_form = "tableswitch on tag"
recovers = "Stmt::MultiArm { ... }"

[version_pin]
javac = "21"
"#;
    let spec = SignatureSpec::parse(toml_str).expect("two-stage form parses");
    assert_eq!(spec.inner_form.as_deref(), Some("tableswitch on tag"));
}

#[test]
fn signature_toml_rejects_missing_required_field() {
    // Missing `recovers` — required.
    let bad = r#"
toolchain = "javac"
version = "21"
construct = "switch_string"
discriminant_kind = "String"
outer_form = "x"
arm_pattern = "y"

[version_pin]
javac = "21"
"#;
    let err = SignatureSpec::parse(bad).expect_err("missing required field rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("recovers") || msg.contains("missing"),
        "error should mention the missing field; got: {msg}",
    );
}

#[test]
fn signature_toml_rejects_missing_version_pin_table() {
    // No `[version_pin]` table — required.
    let bad = r#"
toolchain = "javac"
version = "21"
construct = "switch_string"
discriminant_kind = "String"
outer_form = "x"
arm_pattern = "y"
recovers = "z"
"#;
    assert!(
        SignatureSpec::parse(bad).is_err(),
        "missing [version_pin] table should be rejected",
    );
}

#[test]
fn corpus_signature_files_parse() {
    let root = corpus_root();
    if !root.is_dir() {
        eprintln!("SKIP: {} not present", root.display());
        return;
    }
    let mut count = 0;
    for entry in walkdir_signature_toml(&root) {
        let raw = std::fs::read_to_string(&entry).expect("read signature.toml");
        let spec = SignatureSpec::parse(&raw)
            .unwrap_or_else(|e| panic!("signature.toml parse failed at {}: {}", entry.display(), e));
        eprintln!(
            "OK: {} ({}-{}/{})",
            entry.display(),
            spec.toolchain,
            spec.version,
            spec.construct,
        );
        count += 1;
    }
    assert!(
        count >= 6,
        "expected ≥6 corpus signature.toml files; found {count}",
    );
}

#[test]
fn corpus_check() {
    let entries = discover_corpus().expect("discover_corpus");
    if entries.is_empty() {
        eprintln!("SKIP: no corpus entries under tests/corpus/clean/");
        return;
    }

    let tools = match Tools::resolve() {
        Some(t) => t,
        None => {
            let outcome = skipped_outcome(
                "corpus_check",
                "javac/d8",
                "javac/d8 not found on PATH or via JAVA_HOME/ANDROID_HOME. \
                 Set JAVA_HOME (JDK 21) and ANDROID_HOME (with build-tools containing d8) \
                 or add javac+d8 to PATH. CI provides both. \
                 kotlinc-1.9 entries additionally need KOTLIN_HOME (e.g. /path/to/kotlinc-1.9.22) \
                 or `kotlinc` on PATH.",
            );
            for w in &outcome.warnings {
                eprintln!("SKIP: {w:?}");
            }
            check_warnings_strict(&[outcome]).expect("strict-warnings gate");
            return;
        }
    };

    let needs_kotlinc = entries
        .iter()
        .any(|e| e.toolchain_dir.starts_with("kotlinc-"));
    if needs_kotlinc && tools.kotlinc.is_none() {
        let outcome = skipped_outcome(
            "corpus_check",
            "kotlinc",
            "kotlinc corpus entries present but kotlinc unavailable. \
             Set KOTLIN_HOME (e.g. /path/to/kotlinc-1.9.22) or add kotlinc to PATH.",
        );
        for w in &outcome.warnings {
            eprintln!("SKIP: {w:?}");
        }
        check_warnings_strict(&[outcome]).expect("strict-warnings gate");
        return;
    }

    let out_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("corpus-out");
    std::fs::create_dir_all(&out_root).expect("mkdir target/corpus-out");

    let mut compiled = 0usize;
    let mut failures = Vec::<String>::new();
    for entry in &entries {
        let entry_out = out_root
            .join(&entry.toolchain_dir)
            .join(&entry.construct)
            .join(&entry.name);
        let class_dir = entry_out.join("classes");
        let dex_dir = entry_out.join("dex");
        std::fs::create_dir_all(&class_dir).expect("mkdir class_dir");
        std::fs::create_dir_all(&dex_dir).expect("mkdir dex_dir");

        let compile_result = if entry.toolchain_dir.starts_with("kotlinc-") {
            kotlinc_compile(&tools, &entry.src, &class_dir)
        } else {
            javac_compile(&tools, &entry.src, &class_dir)
        };
        if let Err(e) = compile_result {
            let compiler = if entry.toolchain_dir.starts_with("kotlinc-") {
                "kotlinc"
            } else {
                "javac"
            };
            failures.push(format!(
                "{} ({}/{}): {}/{}: {}",
                entry.src.display(),
                entry.toolchain_dir,
                entry.construct,
                compiler,
                e.stage,
                e.detail,
            ));
            continue;
        }
        if let Err(e) = d8_convert(&tools, &class_dir, &dex_dir) {
            failures.push(format!(
                "{} ({}/{}): d8/{}: {}",
                entry.src.display(),
                entry.toolchain_dir,
                entry.construct,
                e.stage,
                e.detail,
            ));
            continue;
        }
        compiled += 1;
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} corpus entries failed to compile:\n  - {}",
            failures.len(),
            entries.len(),
            failures.join("\n  - "),
        );
    }

    assert!(
        compiled >= 6,
        "expected ≥6 corpus entries to compile; got {compiled}",
    );
    eprintln!(
        "corpus_check: {compiled}/{} entries compiled javac→d8 cleanly",
        entries.len(),
    );
}
