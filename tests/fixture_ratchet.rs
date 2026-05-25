//! Tier-1 language-coverage fixture ratchet for `droidsaw-dex`.
//!
//! For each entry in `tests/fixtures/manifest.toml`, runs the full
//! javac → d8 → decompile → javac → java roundtrip via
//! [`droidsaw_fixture_harness::run_fixture`]. Asserts [`RatchetResult::is_clean`]
//! — any `SemanticFail`, `ResourceLimitExceeded`, unknown/missing fixture,
//! or `CompilePass`↔`CompileFail` drift fails the gate.
//!
//! Tools: `javac` (≥ 17 recommended, runs with `--release 8`), `java`, and
//! `d8` (Android SDK build-tools). `d8` is discovered via `ANDROID_HOME` or
//! `PATH`; when any tool is missing the test skips with `eprintln` rather
//! than hard-failing — mirrors the `tests/dex_roundtrip.rs` and
//! `droidsaw-bench` consumer patterns.
//!
//! Serial execution: the harness installs `setrlimit(RLIMIT_AS)` which is
//! process-global, so fixtures run in a single `#[test]` that iterates the
//! manifest sequentially.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use droidsaw_fixture_harness::{
    FixtureOutcome, Improvement, JvmCaps, Manifest, OutcomeKind, Regression, ResourceCaps, Runner,
    RunnerKind, check_ratchet, check_warnings_strict, run_fixture, skipped_outcome,
};

/// Marker inserted at the start of each decompiled class in the combined
/// `decompile()` output so `recompile()` can split them back into per-file
/// sources. Leading newline is stripped for the very first class; delimiter
/// includes a trailing space then the class name on the same line.
///
/// Hoisted to `droidsaw-fixture-harness::fixture_delimiter_prefix` —
/// the harness-side helper is the canonical source of truth. Binding
/// adapts the runtime-String shape to per-function `&str` use.
fn class_delim() -> String {
    droidsaw_fixture_harness::fixture_delimiter_prefix("dex", "class")
}

/// Per-fixture wall-time cap enforced by the harness.
const PER_FIXTURE_WALL_TIME: Duration = Duration::from_secs(120);

/// Per-fixture native RSS cap (unused for `RunnerKind::Jvm`; kept as a
/// sentinel because `ResourceCaps::rss_bytes`
/// is still required by the struct shape and is consulted only on the
/// Native/Managed install path). The JVM cap model lives in
/// [`JvmCaps`]: `as_reserve = u64::MAX` (the harness clamps to
/// `prior_hard`) maintains the "wall-time is the enforced boundary;
/// RSS is left soft" posture, but unlike the native field it now carries
/// the documented reason — modern OpenJDK reserves tens of GiB of virtual
/// AS for a trivial program (VmPeak ~33.5 GiB on the dev box), so any
/// tighter AS cap kills `javac`/`java`/`d8` at startup with mmap ENOMEM.
const PER_FIXTURE_RSS: u64 = u64::MAX;

/// Wall-time budget for the forked `java` process in `run`/`run_recompiled`.
/// Distinct from the harness's outer cap: this bounds one subprocess, while
/// the outer cap bounds the whole fixture (compile + run + decompile + …).
const JAVA_RUN_TIMEOUT: Duration = Duration::from_secs(20);

#[test]
fn fixture_ratchet() {
    let tools = Tools::resolve();

    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_root = crate_dir.join("tests/fixtures");
    let manifest_path = fixtures_root.join("manifest.toml");
    let manifest = Manifest::load(&manifest_path)
        .unwrap_or_else(|e| panic!("load manifest at {manifest_path:?}: {e}"));

    let caps = ResourceCaps {
        wall_time: PER_FIXTURE_WALL_TIME,
        rss_bytes: PER_FIXTURE_RSS,
        kind: RunnerKind::Jvm,
        jvm: JvmCaps::default(),
    };

    let mut outcomes: Vec<FixtureOutcome> = Vec::with_capacity(manifest.fixtures.len());
    for entry in &manifest.fixtures {
        let outcome = match &tools {
            Some(t) => {
                let runner = DexFixtureRunner {
                    tools: t.clone(),
                    main_class: entry.name.clone(),
                    d8_min_api: entry.d8_min_api,
                };
                run_fixture(runner, entry, &fixtures_root, caps)
            }
            None => skipped_outcome(
                entry.name.clone(),
                "javac/java/d8",
                "javac/java/d8 not found; see tests/README.md for toolchain setup",
            ),
        };
        report(&outcome);
        outcomes.push(outcome);
    }

    let result = check_ratchet(&manifest, &outcomes);
    assert!(
        result.is_clean(),
        "dex fixture ratchet: {} regression(s), {} improvement(s):\n{}",
        result.regressions.len(),
        result.improvements.len(),
        format_findings(&result.regressions, &result.improvements),
    );

    eprintln!(
        "dex fixture ratchet: {}/{} clean ({} skipped)",
        result.unchanged,
        manifest.fixtures.len(),
        result.skipped,
    );

    check_warnings_strict(&outcomes).expect("strict-warnings gate");
}

fn report(outcome: &FixtureOutcome) {
    let tag = match &outcome.kind {
        OutcomeKind::CompilePass => "PASS",
        OutcomeKind::CompileFail { .. } => "COMPILE_FAIL",
        OutcomeKind::SemanticFail { .. } => "SEMANTIC_FAIL",
        OutcomeKind::ResourceLimitExceeded { .. } => "LIMIT",
        OutcomeKind::FixtureReadError { .. } => "READ_ERR",
    };
    eprintln!(
        "  {tag:<13} {} ({:.2}s)",
        outcome.name,
        outcome.wall_time.as_secs_f32()
    );
    for w in &outcome.warnings {
        eprintln!("    warn: {w:?}");
    }
}

fn format_findings(regressions: &[Regression], improvements: &[Improvement]) -> String {
    let mut s = String::new();
    for r in regressions {
        s.push_str(&format!("  - {r:?}\n"));
    }
    for i in improvements {
        s.push_str(&format!("  + {i:?} (update manifest status to compile_pass)\n"));
    }
    s
}

// ─────────────────────────────────────────────────────────────────────────────
// Runner
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Tools {
    javac: PathBuf,
    java: PathBuf,
    d8: PathBuf,
}

impl Tools {
    fn resolve() -> Option<Self> {
        Some(Self {
            javac: resolve_on_path("javac")?,
            java: resolve_on_path("java")?,
            d8: resolve_d8()?,
        })
    }
}

fn resolve_on_path(name: &str) -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(&home).join("bin").join(name);
        if p.exists() {
            return Some(p);
        }
    }
    let out = Command::new("which").arg(name).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(PathBuf::from(s)) }
}

fn resolve_d8() -> Option<PathBuf> {
    if let Ok(android) = std::env::var("ANDROID_HOME") {
        let bt = PathBuf::from(&android).join("build-tools");
        if let Ok(entries) = std::fs::read_dir(&bt) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            versions.sort();
            versions.reverse();
            for v in versions {
                let d8 = v.join("d8");
                if d8.exists() {
                    return Some(d8);
                }
            }
        }
    }
    resolve_on_path("d8")
}

struct DexFixtureRunner {
    tools: Tools,
    main_class: String,
    /// Per-fixture `--min-api N` forwarded to d8. `None` preserves d8's
    /// built-in default (pre-O ≈ API 23); `Some(26)` keeps `invoke-custom`
    /// / `invoke-polymorphic` in the output rather than letting d8 desugar
    /// them to synthetic throw-stubs. Source: `FixtureEntry.d8_min_api`.
    d8_min_api: Option<u32>,
}

struct DexArtifact {
    _tmp: tempfile::TempDir,
    class_dir: PathBuf,
    /// `Some` after `compile_source`; `None` after `recompile` (we don't
    /// re-dex the roundtripped output because `run_recompiled` only needs
    /// `.class` files).
    dex_path: Option<PathBuf>,
}

#[derive(Debug)]
enum DexFixtureError {
    Io { ctx: &'static str, error: String },
    JavacFailed { stderr: String },
    D8Failed { stderr: String },
    DexParse { message: String },
    EmptyDecompile,
    JavaFailed { exit: Option<i32>, stderr: String },
    JavaTimeout,
    RecompileInputInvalid { reason: &'static str },
    MissingDex,
}

impl std::fmt::Display for DexFixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { ctx, error } => write!(f, "io[{ctx}]: {error}"),
            Self::JavacFailed { stderr } => write!(f, "javac: {}", truncate(stderr, 512)),
            Self::D8Failed { stderr } => write!(f, "d8: {}", truncate(stderr, 512)),
            Self::DexParse { message } => write!(f, "dex parse: {}", truncate(message, 512)),
            Self::EmptyDecompile => f.write_str("decompile produced no classes"),
            Self::JavaFailed { exit, stderr } => {
                write!(f, "java exit={exit:?}: {}", truncate(stderr, 512))
            }
            Self::JavaTimeout => f.write_str("java runtime exceeded per-process timeout"),
            Self::RecompileInputInvalid { reason } => {
                write!(f, "recompile input invalid: {reason}")
            }
            Self::MissingDex => f.write_str("decompile called on artifact without a .dex"),
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
}

impl DexFixtureRunner {
    fn javac_compile(
        &self,
        class_dir: &Path,
        sources: &[PathBuf],
    ) -> Result<(), DexFixtureError> {
        let mut cmd = Command::new(&self.tools.javac);
        cmd.arg("--release")
            .arg("8")
            .arg("-Xlint:none")
            .arg("-d")
            .arg(class_dir);
        for s in sources {
            cmd.arg(s);
        }
        let out = cmd.output().map_err(|e| DexFixtureError::Io {
            ctx: "javac spawn",
            error: e.to_string(),
        })?;
        if !out.status.success() {
            return Err(DexFixtureError::JavacFailed {
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        Ok(())
    }

    fn d8_convert(&self, class_dir: &Path, dex_dir: &Path) -> Result<(), DexFixtureError> {
        let entries = std::fs::read_dir(class_dir).map_err(|e| DexFixtureError::Io {
            ctx: "read class_dir",
            error: e.to_string(),
        })?;
        let class_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("class"))
            .collect();
        let mut cmd = Command::new(&self.tools.d8);
        cmd.arg("--no-desugaring");
        if let Some(min_api) = self.d8_min_api {
            cmd.arg("--min-api").arg(min_api.to_string());
        }
        cmd.arg("--output").arg(dex_dir).args(&class_files);
        let out = cmd.output().map_err(|e| DexFixtureError::Io {
            ctx: "d8 spawn",
            error: e.to_string(),
        })?;
        if !out.status.success() {
            return Err(DexFixtureError::D8Failed {
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        Ok(())
    }

    fn run_java(&self, class_dir: &Path) -> Result<String, DexFixtureError> {
        let mut child = Command::new(&self.tools.java)
            .arg("-cp")
            .arg(class_dir)
            .arg(&self.main_class)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DexFixtureError::Io {
                ctx: "java spawn",
                error: e.to_string(),
            })?;
        let started = Instant::now();
        let timed_out = loop {
            match child.try_wait() {
                Ok(Some(_)) => break false,
                Ok(None) if started.elapsed() >= JAVA_RUN_TIMEOUT => {
                    let _ = child.kill();
                    break true;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(e) => {
                    return Err(DexFixtureError::Io {
                        ctx: "java try_wait",
                        error: e.to_string(),
                    });
                }
            }
        };
        let out = child.wait_with_output().map_err(|e| DexFixtureError::Io {
            ctx: "java wait_with_output",
            error: e.to_string(),
        })?;
        if timed_out {
            return Err(DexFixtureError::JavaTimeout);
        }
        if !out.status.success() {
            return Err(DexFixtureError::JavaFailed {
                exit: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl Runner for DexFixtureRunner {
    type Artifact = DexArtifact;
    type Error = DexFixtureError;

    fn compile_source(&self, source: &str) -> Result<DexArtifact, DexFixtureError> {
        let tmp = tempfile::tempdir().map_err(|e| DexFixtureError::Io {
            ctx: "tempdir",
            error: e.to_string(),
        })?;
        let src_dir = tmp.path().join("src");
        let class_dir = tmp.path().join("classes");
        let dex_dir = tmp.path().join("dex");
        for d in [&src_dir, &class_dir, &dex_dir] {
            std::fs::create_dir_all(d).map_err(|e| DexFixtureError::Io {
                ctx: "mkdir scratch",
                error: e.to_string(),
            })?;
        }
        let src_path = src_dir.join(format!("{}.java", self.main_class));
        std::fs::write(&src_path, source).map_err(|e| DexFixtureError::Io {
            ctx: "write src",
            error: e.to_string(),
        })?;
        self.javac_compile(&class_dir, &[src_path])?;
        self.d8_convert(&class_dir, &dex_dir)?;
        Ok(DexArtifact {
            _tmp: tmp,
            class_dir,
            dex_path: Some(dex_dir.join("classes.dex")),
        })
    }

    fn run(&self, artifact: &DexArtifact) -> Result<String, DexFixtureError> {
        self.run_java(&artifact.class_dir)
    }

    fn decompile(&self, artifact: &DexArtifact) -> Result<String, DexFixtureError> {
        let dex_path = artifact.dex_path.as_ref().ok_or(DexFixtureError::MissingDex)?;
        let data = std::fs::read(dex_path).map_err(|e| DexFixtureError::Io {
            ctx: "read dex",
            error: e.to_string(),
        })?;
        let dex = droidsaw_dex::DexFile::parse(&data, None)
            .map_err(|e| DexFixtureError::DexParse { message: format!("{e:?}") })?;
        let delim = class_delim();
        let mut out = String::new();
        let mut any = false;
        // `classes_to_decompile` pre-filters out javac/d8 enum artifacts
        // that should not have a separate decompile output (per-constant
        // subclass bodies + d8 anonymous-marker `-IA` stubs).
        // Per-constant body content is re-introduced by inlining into
        // the parent enum's constant declarations via `EnumInlineMap` +
        // `decompile_class_ext`.
        //
        // Nested classes (`Outer$Inner`, `Outer$1`, `Outer$Op$1`) that
        // aren't suppressible-enum-artifacts still decompile into their
        // own per-file output: javac accepts `$` as a source-level
        // identifier character, so the per-class file form handles
        // StaticNestedClass / AnonymousInnerClass / LocalInnerClass /
        // EnumWithMethods without requiring structural nesting.
        let ttm = droidsaw_dex::classes::TypeToClassDefMap::build(&dex);
        let enum_inlines = droidsaw_dex::classes::EnumInlineMap::build(&dex, &data, &ttm);
        for (class_idx, class_def) in droidsaw_dex::classes::classes_to_decompile(&dex) {
            let descriptor = dex.get_type_descriptor(class_def.class_idx).unwrap_or("?");
            let class_name = descriptor
                .strip_prefix('L')
                .unwrap_or(descriptor)
                .strip_suffix(';')
                .unwrap_or(descriptor)
                .to_string();
            let _ = class_idx; // unused; retained for clarity
            let source = droidsaw_dex::classes::decompile_class_ext(
                &dex,
                &data,
                class_def,
                Some(&enum_inlines),
                Some(&ttm),
            );
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&delim);
            out.push_str(&class_name);
            out.push('\n');
            out.push_str(&source);
            any = true;
        }
        if !any {
            return Err(DexFixtureError::EmptyDecompile);
        }
        Ok(out)
    }

    fn recompile(&self, decompiled: &str) -> Result<DexArtifact, DexFixtureError> {
        let delim = class_delim();
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut remaining = decompiled;
        while let Some(marker) = remaining.find(&delim) {
            let after = &remaining[marker + delim.len()..];
            let (name_line, body_start) = match after.find('\n') {
                Some(nl) => (&after[..nl], &after[nl + 1..]),
                None => (after, ""),
            };
            let name = name_line.trim().to_string();
            if name.is_empty() {
                return Err(DexFixtureError::RecompileInputInvalid {
                    reason: "empty class name after delimiter",
                });
            }
            let next = body_start.find(&delim);
            let (body, rest) = match next {
                Some(off) => (&body_start[..off], &body_start[off..]),
                None => (body_start, ""),
            };
            pairs.push((name, body.trim_end_matches('\n').to_string()));
            remaining = rest;
        }
        if pairs.is_empty() {
            return Err(DexFixtureError::RecompileInputInvalid {
                reason: "no class delimiters found",
            });
        }
        let tmp = tempfile::tempdir().map_err(|e| DexFixtureError::Io {
            ctx: "tempdir",
            error: e.to_string(),
        })?;
        let src_dir = tmp.path().join("src");
        let class_dir = tmp.path().join("classes");
        for d in [&src_dir, &class_dir] {
            std::fs::create_dir_all(d).map_err(|e| DexFixtureError::Io {
                ctx: "mkdir scratch",
                error: e.to_string(),
            })?;
        }
        let mut sources: Vec<PathBuf> = Vec::with_capacity(pairs.len());
        for (name, body) in &pairs {
            let simple = name.rsplit('/').next().unwrap_or(name);
            let p = src_dir.join(format!("{simple}.java"));
            std::fs::write(&p, body).map_err(|e| DexFixtureError::Io {
                ctx: "write decompiled",
                error: e.to_string(),
            })?;
            sources.push(p);
        }
        self.javac_compile(&class_dir, &sources)?;
        Ok(DexArtifact {
            _tmp: tmp,
            class_dir,
            dex_path: None,
        })
    }

    fn run_recompiled(&self, artifact: &DexArtifact) -> Result<String, DexFixtureError> {
        self.run_java(&artifact.class_dir)
    }
}

