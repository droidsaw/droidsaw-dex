# section_offset_overlap — DEX id-section overlap fixture

Adversarial fixture for the DEX id-section **overlap** class: a header whose six
id-section offsets alias each other, so the same bytes decode as two different
item types. Guards `DexError::SectionOverlap` (parser hard-reject) and the
`section_overlap_paired_*` test in `tests/negative_properties.rs`.

## Files
| File | sha256 | role |
|---|---|---|
| `src.java` | — | subject, default package, `public class SectionOverlap` (`harmless()`, `sensitive()` → `Runtime.exec`, static field `marker`) |
| `build.sh` | — | generator (mirrors `tests/fixtures/build_fixture.sh`: `ANDROID_HOME` discovery, `javac -g --release 8`, `d8 --no-desugaring`) |
| `base.dex` | `01ce1837713ace6eed389b3aa9eb06c30a5b5c63054618eab56a6a41d0e53d8c` | benign — parses Ok with the true inventory |
| `method_ids_aliases_field_ids.dex` | `f97473167176279d98ff31450708ea908122d2bc84472ed769937878c294935c` | adversarial — `method_ids_off := field_ids_off`, Adler-32 re-sealed |

`marker` is load-bearing: a static `int` field's `field_id_item` is the same
8-byte shape as a `method_id_item`, so aliasing `method_ids` onto `field_ids`
decodes the field row **as** a method.

## Reproduce
```
ANDROID_HOME=~/Library/Android/sdk ./build.sh
```
Rebuilds `base.dex` from `src.java`, then applies the one-field mutation
inline (`method_ids_off := field_ids_off` + Adler-32 reseal). The mutation
touches exactly two header words: `method_ids_off` (@0x5c) and `checksum` (@0x08).
The SHA-1 signature is **not** re-sealed — `DexFile::parse` ignores it; an
on-device test would need to re-sign.

## Cross-tool differential

| Reader | version | `method_ids_aliases_field_ids.dex` | observed |
|---|---|---|---|
| **dexdump** | SDK build-tools 35.0.0 | **REJECT** (exit 1) | `Failure to verify dex file …: Bad offset for section: got 138, expected 130` |
| **droidsaw** (without the overlap guard) | — | **ACCEPT, silent** | `marker` (a field) reported as a method, `Runtime.getRuntime` dropped; `audit` basic = 0 findings |
| **droidsaw** (with the overlap guard) | — | **REJECT** | `id-section overlap: field_ids [0x130..0x138) overlaps method_ids [0x130..0x170)` |
| **jadx** | 1.5.5 | **ACCEPT, errored** | scrambled decompile + internal `IllegalArgumentException`, exit 3 |
| apktool / dexlib2 | 3.0.2 | inconclusive | requires a zip/APK wrapper, not raw dex |

`base.dex` is accepted by all of the above (dexdump exit 0; droidsaw lists the
true `SectionOverlap.{harmless,sensitive}` + `Runtime.exec`).

## Honest scope
**Not a runtime-evasion primitive.** dexdump runs the AOSP `DexFileVerifier`
(the family ART uses at load / dex2oat) and hard-rejects the overlap — a real
device won't run the file, so nothing malicious can hide in it. The finding is
the inverse: **without the overlap guard droidsaw was uniquely silent** (dexdump
rejects, jadx errors, droidsaw alone accepted + 0 findings). The guard **aligns
droidsaw with the AOSP reference verifier** — analysis-integrity hardening
validated against the canonical implementation.

AOSP is in fact *stricter*: `expected 130` shows it enforces a full **contiguous
tiling** (each section exactly after the previous), so it also rejects
gaps/reorder. The fix deliberately catches only the **overlap** subset (the
aliasing that yields a wrong IR), preserving droidsaw's tolerance for
non-overlapping noncanonical files a runtime would refuse — a chosen narrower
invariant.

The genuine evasion candidate (a file ART *accepts* but droidsaw *misreads* —
header/map disagreement, or AOSP-tolerated quirks) is **open, on-device**:
- E1: does ART's verifier accept the overlap, or reject it (as dexdump does)?
- E2: does ART resolve `method_ids` from the header or the `map_list`?
- E3: Frida-confirm whether a sensitive call executes while droidsaw shows benign.
