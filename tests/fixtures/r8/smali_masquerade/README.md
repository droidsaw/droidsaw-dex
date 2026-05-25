# R8 smali-forged masquerade (adversarial PoC, positive control)

**What this fixture demonstrates:** an attacker who controls bytecode
emit can defeat *every* bytecode-level gate the R8 outline-helper
recogniser uses (the `KNOWN_FP_FAMILY` family-prefix filter, the
`ACC_SYNTHETIC` class-flag gate, the `$$ExternalSyntheticOutline`
descriptor-allow-list, and the structural predicates I4–I13). The
recogniser still fires — but at the low-trust `StructurallyOutlineLike`
variant, not at `BlockOutlinedHelper` with `mapping_confirmed: true`.

The marker is *honest about its uncertainty.* The actual security
boundary is the paired-mapping oracle (`elevate_with_oracle`): an
attacker without control of the R8 toolchain cannot forge the
`com.android.tools.r8.outline` annotation in the mapping.txt that
elevates the marker. That stays the same after this PoC.

## Why "smali"

The brief that scoped this fixture used "smali" as shorthand for direct
bytecode authoring — the Android-SDK assembler that takes `.smali` text
and produces DEX bytes. The fixture authors bytes directly, but does it
via droidsaw-dex's own `emit_dex::emit_dex` (in-process mutation of a
parsed `DexFile` + re-emit) rather than shelling out to the smali jar.
The mechanic is equivalent — every byte that ends up in the emitted DEX
is under the test's control — and the in-process path avoids a CI
dependency on the Android SDK.

## Pair with `family_prefix_masquerade/`

This fixture is the **positive control** to `family_prefix_masquerade/`'s
**negative control**:

| | `family_prefix_masquerade/` | `smali_masquerade/` (this fixture) |
|---|---|---|
| **Source** | kotlinc + R8 (real toolchain) | in-process mutation of `tests/fixtures/classes_named.dex` |
| **Forges `ACC_SYNTHETIC`?** | No (kotlinc cannot) | Yes (in-place `&#124;= 0x1000` on `class_def.access_flags`) |
| **Forges descriptor suffix?** | No (kotlinc keeps `LBackdoor;`) | Yes (`LMinimalNamed;` → `Landroidx/internal/Forged$$ExternalSyntheticOutline0;`) |
| **Recogniser fires?** | No — `ACC_SYNTHETIC` gate rejects | Yes — every bytecode-level gate passes |
| **Marker variant if firing?** | (n/a, doesn't fire) | `StructurallyOutlineLike` (low-trust) |
| **Mapping required to elevate?** | (n/a) | Yes — `elevate_with_oracle` is the un-bypassed boundary |
| **Demonstrates** | the `ACC_SYNTHETIC` gate works | the gate does NOT bound a bytecode-controlling attacker |

Both PoCs share the **lens-blinding** framing: the masquerade affects
recogniser summary statistics, not the actual byte content of the DEX.
A `strings` pass over the emitted DEX still surfaces the `"PWNED:smali-
masquerade"` witness — the malicious code is fully inspectable, only the
`marker_count` / `off_family_marker_count` aggregates are wrong.

## Threat model

The recogniser's gates are FP-reduction heuristics tuned against
real-world R8 output, not access controls. The fixture proves:

1. **Family-prefix is opportunistic.** `Landroidx/internal/Forged...` is
   lexically in the `androidx` `KNOWN_FP_FAMILY` bucket. DEX has no
   namespace ownership enforcement; the masquerade just declares
   itself in that namespace.

2. **`ACC_SYNTHETIC` is forgeable.** ART trusts the flag; no runtime
   check distinguishes "R8 set this" from "an attacker set this."

3. **Descriptor allow-list is enumerable.** The recogniser admits
   `Outline`, `CovariantOutline`, `ApiModelOutline`,
   `NonStartupInStartupOutline`, `BUOutline`, `ObjectCloneOutline`
   suffixes. The attacker just picks one.

4. **Structural predicates are forgeable.** No fields, single static
   method, outline-eligible body, ≥ 2 distinct invoke-static callers
   — all author-controlled.

5. **The marker variant is the un-forgeable signal.** Without an
   `OutlineOracle` populated from a real `mapping.txt`, the
   recogniser emits `StructurallyOutlineLike` — explicitly low-trust.
   Consumers who care about high-trust attestation MUST pair with a
   mapping; bytecode alone cannot reach `BlockOutlinedHelper {
   mapping_confirmed: true }`.

## Mitigation guidance

The recogniser already does the right thing — it emits a low-trust
marker on adversarial input. The mitigation is downstream of this PoC:

- **Mapping-paired analysis** (product corpus with R8 build outputs).
  The `com.android.tools.r8.outline` annotation in mapping.txt is the
  ground-truth gauge. Attackers without R8-toolchain access cannot
  forge it.

- **For mapping-less analysis** (production APKs, F-Droid sweeps),
  treat `StructurallyOutlineLike` markers as *signal, not attestation*.
  The `namespace_rollup_attested` ratio in the F-Droid harness is the
  masquerade-window diagnostic: a low attested fraction in a bucket
  signals a wide masquerade window in that namespace.

- **Do not promote `StructurallyOutlineLike` to `BlockOutlinedHelper`
  without an oracle.** This fixture is the regression sentinel for that
  invariant; if a future commit elevates the variant on bytecode-only
  input, this fixture fails.

## Layout

This fixture has no `source/` or `scripts/` directory — the forgery is
implemented purely as test-time mutation in
`tests/r8_smali_masquerade_fixture_ratchet.rs`. The "base" DEX it
mutates is the existing repo-wide minimal fixture
`tests/fixtures/classes_named.dex` (built from `MinimalNamed.java` by
`tests/regen.sh` — same upstream as the rest of the test suite). No
new build infrastructure is required.

## Assertions (positive control: forgery succeeds and recogniser is honest)

1. **Forged DEX round-trips.** `emit_dex` produces bytes that
   `DexFile::parse` accepts; mutated descriptor and `ACC_SYNTHETIC`
   bit survive the round-trip.

2. **Recogniser fires exactly one marker** on the masquerade class
   `Landroidx/internal/Forged$$ExternalSyntheticOutline0;`, method
   `accumulate`.

3. **Marker variant is `StructurallyOutlineLike`.** The load-bearing
   assertion of this fixture. If this assertion ever fires
   `BlockOutlinedHelper`, the contract that elevation requires a
   paired-mapping oracle has regressed.

4. **Confidence is 40.** Floor tier of
   `block_outlined_ladder_confidence` at 2 distinct callers.

5. **`"PWNED:smali-masquerade"` survives in the DEX string table.**
   Lens-blinding-not-code-hiding witness.

## Cross-references

- `tests/fixtures/r8/family_prefix_masquerade/README.md` — paired
  negative-control PoC.
- `src/r8_inversion.rs::recognise_outline_helper_v2` — the recogniser
  whose gates this fixture exercises.
- `src/r8_inversion.rs::elevate_with_oracle` — the mapping-paired
  elevation step that this PoC explicitly does NOT bypass.
- `tests/r8_fdroid_apk_sweep.rs::KNOWN_FP_FAMILY` — full threat-model
  discussion of the masquerade window in mapping-less analysis.
