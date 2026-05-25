//! DEX round-trip: javac → d8 → decompile → javac → run → compare.
//!
//! For each Java fixture in `tests/fixtures/java/`, this test:
//!   1. Compiles `<Fixture>.java` with `javac --release 8`
//!   2. Converts ALL resulting `.class` files to `.dex` with `d8 --no-desugaring`
//!   3. Calls `droidsaw_dex::classes::decompile_class` on every non-inner class
//!   4. Writes each decompiled class to its own `.java` file, compiles them together
//!   5. If compilation succeeds, runs `java` and compares stdout against the
//!      expected output stored in the FIXTURES table
//!
//! A fixture that fails `javac` → COMPILE_FAIL.
//! A fixture that produces wrong output → SEMANTIC_FAIL.
//! The test asserts zero SEMANTIC_FAILs; COMPILE_FAILs are reported but
//! tolerated while the decompiler matures.
//!
//! Run:
//!   cargo test --test dex_roundtrip -- --nocapture

use std::path::{Path, PathBuf};
use std::process::Command;

use droidsaw_fixture_harness::{check_warnings_strict, skipped_outcome};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/java")
}

fn resolve_bin(name: &str) -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(&home).join("bin").join(name);
        if p.exists() {
            return Some(p);
        }
    }
    let out = Command::new("which").arg(name).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

fn resolve_d8() -> Option<PathBuf> {
    // Check ANDROID_HOME/build-tools/*/d8
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
    // Fallback: PATH
    let out = Command::new("which").arg("d8").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

struct Fixture {
    name: &'static str,
    expected: &'static str,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "Arithmetic",
        expected: "7\n42\n2\n3000000000",
    },
    Fixture {
        name: "ControlFlow",
        expected: "pos\nneg\nzero\n55\n111\nTue\nother",
    },
    Fixture {
        name: "StringOps",
        expected: "ababab\n3\nHELLO\n42",
    },
    Fixture {
        name: "ArrayOps",
        expected: "15\n5 4 3 2 1 \n60",
    },
    Fixture {
        name: "Exceptions",
        expected: "42\n-1\n5\ndivzero",
    },
    Fixture {
        name: "Bits",
        expected: "8\n0\n2\nhEL",
    },
    Fixture {
        name: "Objects",
        expected: "5\ntrue\nfalse\n5",
    },
    Fixture {
        name: "Numerics",
        expected: "1\n3\n32\n3\n15\n5050\n3.0",
    },
    Fixture {
        name: "Casts",
        expected: "10.5\n100003\n65 65 A\n2.5\n42.0\n12345",
    },
    Fixture {
        name: "BitsWide",
        expected: "8\n190\n65451\n8589934590\n16\n2565",
    },
    Fixture {
        name: "StaticState",
        expected: "0\n3\n21\nval:42",
    },
    Fixture {
        name: "ForEach",
        expected: "14\n5\n2\nhello world",
    },
    Fixture {
        name: "Ternary",
        expected: "7\n3\n5\n0\n10\n3\n5",
    },
    Fixture {
        name: "MultiCatch",
        expected: "int:42\nbad:abc\n3\n-1\nhel\nerr\nerr",
    },
    Fixture {
        name: "LogicalOps",
        expected: "true\nfalse\ntrue\nfalse\ntrue\nfalse",
    },
    Fixture {
        name: "DoWhile",
        expected: "60\n27",
    },
    Fixture {
        name: "InstanceOf",
        expected: "str:5\nint:42\nnull\nother\n6",
    },
    Fixture {
        name: "InnerClass",
        expected: "11\n15",
    },
    Fixture {
        name: "EarlyReturn",
        expected: "5\n-1\n2\n-1",
    },
    Fixture {
        name: "MultiArray",
        expected: "1 0\n1\n2 3 4\n21",
    },
];

/// Decompile every non-inner class in the DEX.
/// Returns a list of (simple_class_name, java_source) pairs.
fn decompile_all(dex_path: &Path) -> Vec<(String, String)> {
    let data = match std::fs::read(dex_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  read dex failed: {e}");
            return vec![];
        }
    };
    let dex = match droidsaw_dex::DexFile::parse(&data, None) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  parse dex failed: {e:?}");
            return vec![];
        }
    };
    let mut results = Vec::new();
    for cls in dex.list_classes() {
        // Skip inner / anonymous / synthetic classes (contain '$')
        if cls.descriptor.contains('$') {
            continue;
        }
        // Descriptor is "LFoo;" or "Lfoo/Bar;" — extract simple or qualified name
        let class_name = cls
            .descriptor
            .strip_prefix('L')
            .unwrap_or(&cls.descriptor)
            .strip_suffix(';')
            .unwrap_or(&cls.descriptor)
            .to_string();
        let class_def = &dex.class_defs[cls.class_idx];
        let source = droidsaw_dex::classes::decompile_class(&dex, &data, class_def);
        results.push((class_name, source));
    }
    results
}

#[test]
fn dex_roundtrip_compiles_and_runs() {
    let (javac, java, d8) = match (resolve_bin("javac"), resolve_bin("java"), resolve_d8()) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        (a, b, c) => {
            let mut skip_outcomes = Vec::new();
            if a.is_none() {
                skip_outcomes.push(skipped_outcome(
                    "dex_roundtrip_compiles_and_runs",
                    "javac",
                    "javac not found",
                ));
            }
            if b.is_none() {
                skip_outcomes.push(skipped_outcome(
                    "dex_roundtrip_compiles_and_runs",
                    "java",
                    "java not found",
                ));
            }
            if c.is_none() {
                skip_outcomes.push(skipped_outcome(
                    "dex_roundtrip_compiles_and_runs",
                    "d8",
                    "d8 not found",
                ));
            }
            for o in &skip_outcomes {
                for w in &o.warnings {
                    eprintln!("SKIP: {w:?}");
                }
            }
            check_warnings_strict(&skip_outcomes).expect("strict-warnings gate");
            return;
        }
    };

    let fdir = fixture_dir();
    let workdir = tempfile::tempdir().expect("tempdir");

    let mut compile_fails = 0usize;
    let mut semantic_fails = 0usize;
    let mut passes = 0usize;

    for fixture in FIXTURES {
        // Per-fixture working dirs — keeps companion classes isolated
        let compile_dir = workdir.path().join(format!("compile_{}", fixture.name));
        let dex_dir = workdir.path().join(format!("dex_{}", fixture.name));
        let recomp_dir = workdir.path().join(format!("recomp_{}", fixture.name));
        for d in [&compile_dir, &dex_dir, &recomp_dir] {
            std::fs::create_dir_all(d).expect("create subdir");
        }

        // ── Step 1: javac original fixture ───────────────────────────
        let src = fdir.join(format!("{}.java", fixture.name));
        if !src.exists() {
            eprintln!("SKIP {}: no .java source", fixture.name);
            continue;
        }

        let orig_compile = Command::new(&javac)
            .arg("--release").arg("8")
            .arg("-d").arg(&compile_dir)
            .arg(&src)
            .output()
            .expect("javac spawn");
        if !orig_compile.status.success() {
            eprintln!(
                "ERROR {}: original javac failed:\n{}",
                fixture.name,
                String::from_utf8_lossy(&orig_compile.stderr)
            );
            continue;
        }

        // ── Step 2: d8 → .dex (all .class files in compile_dir) ─────
        let class_files: Vec<PathBuf> = std::fs::read_dir(&compile_dir)
            .expect("read compile_dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("class"))
            .collect();

        let d8_out = Command::new(&d8)
            .arg("--no-desugaring")
            .arg("--output").arg(&dex_dir)
            .args(&class_files)
            .output()
            .expect("d8 spawn");
        if !d8_out.status.success() {
            eprintln!(
                "ERROR {}: d8 failed:\n{}",
                fixture.name,
                String::from_utf8_lossy(&d8_out.stderr)
            );
            continue;
        }

        let fixture_dex = dex_dir.join("classes.dex");

        // ── Step 3: decompile all non-inner classes ───────────────────
        let classes = decompile_all(&fixture_dex);
        if classes.is_empty() {
            eprintln!("FAIL {}: decompile returned no classes", fixture.name);
            compile_fails += 1;
            continue;
        }

        // ── Step 4: write sources, compile together ───────────────────
        let mut java_files: Vec<PathBuf> = Vec::new();
        for (class_name, source) in &classes {
            // class_name may be "Foo" or "foo/Bar"; use last component as filename
            let simple = class_name.rsplit('/').next().unwrap_or(class_name);
            let src_path = recomp_dir.join(format!("{simple}.java"));
            std::fs::write(&src_path, source).expect("write recompile src");
            java_files.push(src_path);
        }

        let recomp_out = Command::new(&javac)
            .arg("--release").arg("8")
            .arg("-Xlint:none")
            .arg("-d").arg(&recomp_dir)
            .args(&java_files)
            .output()
            .expect("javac recompile spawn");

        if !recomp_out.status.success() {
            let stderr = String::from_utf8_lossy(&recomp_out.stderr);
            let short: String = stderr.lines().take(10).collect::<Vec<_>>().join("\n");
            eprintln!("COMPILE_FAIL {}:\n{short}", fixture.name);
            // Print all decompiled sources for debugging
            for (class_name, source) in &classes {
                eprintln!("--- decompiled {class_name} ---");
                for (i, line) in source.lines().enumerate() {
                    eprintln!("{:4}: {line}", i + 1);
                }
            }
            eprintln!("--- end ---");
            compile_fails += 1;
            continue;
        }

        // ── Step 5: run and compare (10-second timeout) ──────────────
        let mut child = Command::new(&java)
            .arg("-cp").arg(&recomp_dir)
            .arg(fixture.name)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("java spawn");

        let timeout = std::time::Duration::from_secs(10);
        let start = std::time::Instant::now();
        let timed_out = loop {
            match child.try_wait().expect("try_wait") {
                Some(_) => break false,
                None if start.elapsed() >= timeout => {
                    let _ = child.kill();
                    break true;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        };
        let run_out = child.wait_with_output().expect("wait_with_output");

        if timed_out {
            eprintln!("SEMANTIC_FAIL {} (timeout — likely infinite loop):", fixture.name);
            for (class_name, source) in &classes {
                eprintln!("--- decompiled {class_name} ---");
                for (i, line) in source.lines().enumerate() {
                    eprintln!("{:4}: {line}", i + 1);
                }
            }
            eprintln!("--- end ---");
            semantic_fails += 1;
            continue;
        }

        let got = String::from_utf8_lossy(&run_out.stdout);
        let got_trimmed = got.trim_end();
        let exp_trimmed = fixture.expected.trim_end();

        if got_trimmed == exp_trimmed {
            eprintln!("PASS {}", fixture.name);
            passes += 1;
        } else {
            eprintln!("SEMANTIC_FAIL {}:", fixture.name);
            eprintln!("  expected: {:?}", &exp_trimmed[..exp_trimmed.len().min(80)]);
            eprintln!("  got:      {:?}", &got_trimmed[..got_trimmed.len().min(80)]);
            for (class_name, source) in &classes {
                eprintln!("--- decompiled {class_name} ---");
                for (i, line) in source.lines().enumerate() {
                    eprintln!("{:4}: {line}", i + 1);
                }
            }
            eprintln!("--- end ---");
            semantic_fails += 1;
        }
    }

    eprintln!(
        "\n=== round-trip summary ===\npasses: {passes}  compile_fails: {compile_fails}  semantic_fails: {semantic_fails}"
    );

    assert_eq!(
        semantic_fails,
        0,
        "{semantic_fails} fixture(s) compiled but produced wrong output"
    );
}
