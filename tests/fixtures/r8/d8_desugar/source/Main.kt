// Engineered Kotlin source for the D8 desugar fixture.
//
// Two purposes, one source file:
//
// 1. Invoke enough java.time + java.util.stream backport API surface
//    that D8 (--min-api 24) desugars the calls into `j$/time/*` and
//    `j$/util/stream/*` backport classes. Each TimeOps and StreamOps
//    method uses 3-4 distinct backported APIs to maximise the j$
//    backport population in the output DEX.
//
// 2. Provide a structurally-outline-like pattern in the NON-j$ code
//    so the ratchet's positive-control assertion has something to
//    fire on. The OutlineLike group has 30 caller methods invoking
//    an identical 4-instruction StringBuilder body — the same shape
//    as the BlockOutline fixture's GroupAInt. R8's outliner extracts
//    the body into a `<context>$$ExternalSyntheticOutline$N` helper
//    in the non-j$ namespace; the ratchet asserts the recogniser
//    fires on that helper (positive control).
//
// Why both in one source file: the structural claim under test is
// "j$/* are NOT outlines". The cleanest empirical anchor is a single
// build that produces BOTH j$ backports AND non-j$ outlines, and
// shows the outline annotations attach only to the latter. A
// two-source-file design would split the artifact set across two
// builds, weakening the demonstration.

package fox.droidsaw.r8fixture.d8desugar

import java.time.LocalDate
import java.time.LocalDateTime
import java.time.Duration
import java.time.format.DateTimeFormatter
import java.util.Optional
import java.util.stream.Collectors
import java.util.stream.IntStream
import java.util.stream.Stream

// ─── java.time backport callers ───
// Each method invokes 3-4 distinct java.time APIs. D8 at --min-api 24
// rewrites these to `j$.time.*` static helpers since java.time is
// API 26+.

object TimeOps {
    @JvmStatic
    fun t01(seed: Int): String {
        val d = LocalDate.now().plusDays(seed.toLong())
        val dt = LocalDateTime.now().minusHours(seed.toLong())
        val fmt = DateTimeFormatter.ISO_LOCAL_DATE_TIME
        return d.format(DateTimeFormatter.ISO_LOCAL_DATE) + "|" + dt.format(fmt)
    }

    @JvmStatic
    fun t02(seed: Int): String {
        val d1 = LocalDate.of(2024, 1, 1).plusMonths(seed.toLong())
        val d2 = LocalDate.of(2024, 12, 31).minusWeeks(seed.toLong())
        val days = d2.toEpochDay() - d1.toEpochDay()
        return "$d1|$d2|$days"
    }

    @JvmStatic
    fun t03(seed: Int): String {
        val dt = LocalDateTime.of(2024, 6, 15, 12, 30).plusMinutes(seed.toLong())
        val duration = Duration.ofSeconds(seed.toLong() * 60L)
        val later = dt.plus(duration)
        return "${dt.year}|${later.dayOfMonth}|${duration.toMinutes()}"
    }

    @JvmStatic
    fun t04(seed: Int): String {
        val d = LocalDate.now().withDayOfMonth(1).plusDays(seed.toLong())
        val fmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")
        return d.format(fmt) + "|" + d.dayOfWeek.name + "|" + d.lengthOfMonth()
    }

    @JvmStatic
    fun t05(seed: Int): String {
        val a = LocalDateTime.now()
        val b = a.plusDays(seed.toLong())
        val between = Duration.between(a, b)
        return "${a.hour}|${b.hour}|${between.toHours()}"
    }
}

// ─── java.util.stream backport callers ───
// D8 at --min-api 24 desugars Stream.collect(Collectors.*) into
// `j$.util.stream.*` since the API surface predates min-api 24 only
// in a partial sense (full stream desugaring is opt-in via the
// desugared-library configuration). With the desugared-library
// config wired in, these calls route through `Lj$/util/stream/*`.

object StreamOps {
    @JvmStatic
    fun s01(n: Int): String {
        val sum = IntStream.range(0, n.coerceAtLeast(1))
            .filter { it % 2 == 0 }
            .sum()
        return "sum=$sum"
    }

    @JvmStatic
    fun s02(n: Int): String {
        val joined = Stream.of("a", "b", "c", "d", "e")
            .limit(n.toLong().coerceAtLeast(1L))
            .collect(Collectors.joining(","))
        return "joined=$joined"
    }

    @JvmStatic
    fun s03(n: Int): String {
        val grouped = IntStream.rangeClosed(1, n.coerceAtLeast(1))
            .boxed()
            .collect(Collectors.groupingBy { it % 3 })
        return "grouped_keys=${grouped.keys.sorted()}"
    }

    @JvmStatic
    fun s04(n: Int): String {
        val list = Stream.of(1, 2, 3, 4, 5)
            .map { it * n }
            .collect(Collectors.toList())
        return "mapped=$list"
    }

    @JvmStatic
    fun s05(n: Int): String {
        val opt: Optional<Int> = IntStream.of(n, n + 1, n + 2)
            .boxed()
            .max(Comparator.naturalOrder())
        return "max=${opt.orElse(-1)}"
    }
}

// ─── Optional + misc backport callers ───

object OptionalOps {
    @JvmStatic
    fun o01(seed: Int): String {
        val o = Optional.of(seed).map { it * 2 }.filter { it > 0 }
        return "o01=${o.orElse(-1)}"
    }

    @JvmStatic
    fun o02(seed: Int): String {
        val a = Optional.ofNullable<Int>(if (seed > 0) seed else null)
        val b = a.map { it + 10 }.orElse(0)
        return "o02=$b"
    }
}

// ─── Structurally-outline-like group ───
// 30 callers invoking an identical 4-instruction StringBuilder body
// with a single Int param. Mirrors the BlockOutline fixture's
// GroupAInt to ensure R8's outliner produces at least one
// non-j$ outline helper — the positive control for the ratchet.

object OutlineLike {
    @JvmStatic fun n01(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|01"); return sb.toString() }
    @JvmStatic fun n02(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|02"); return sb.toString() }
    @JvmStatic fun n03(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|03"); return sb.toString() }
    @JvmStatic fun n04(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|04"); return sb.toString() }
    @JvmStatic fun n05(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|05"); return sb.toString() }
    @JvmStatic fun n06(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|06"); return sb.toString() }
    @JvmStatic fun n07(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|07"); return sb.toString() }
    @JvmStatic fun n08(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|08"); return sb.toString() }
    @JvmStatic fun n09(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|09"); return sb.toString() }
    @JvmStatic fun n10(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|10"); return sb.toString() }
    @JvmStatic fun n11(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|11"); return sb.toString() }
    @JvmStatic fun n12(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|12"); return sb.toString() }
    @JvmStatic fun n13(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|13"); return sb.toString() }
    @JvmStatic fun n14(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|14"); return sb.toString() }
    @JvmStatic fun n15(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|15"); return sb.toString() }
    @JvmStatic fun n16(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|16"); return sb.toString() }
    @JvmStatic fun n17(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|17"); return sb.toString() }
    @JvmStatic fun n18(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|18"); return sb.toString() }
    @JvmStatic fun n19(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|19"); return sb.toString() }
    @JvmStatic fun n20(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|20"); return sb.toString() }
    @JvmStatic fun n21(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|21"); return sb.toString() }
    @JvmStatic fun n22(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|22"); return sb.toString() }
    @JvmStatic fun n23(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|23"); return sb.toString() }
    @JvmStatic fun n24(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|24"); return sb.toString() }
    @JvmStatic fun n25(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|25"); return sb.toString() }
    @JvmStatic fun n26(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|26"); return sb.toString() }
    @JvmStatic fun n27(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|27"); return sb.toString() }
    @JvmStatic fun n28(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|28"); return sb.toString() }
    @JvmStatic fun n29(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|29"); return sb.toString() }
    @JvmStatic fun n30(x: Int): String { val sb = StringBuilder(); sb.append("N:"); sb.append(x); sb.append("|30"); return sb.toString() }
}

object Main {
    // Entry point references every helper so R8 cannot tree-shake.
    @JvmStatic
    fun main(args: Array<String>) {
        val out = StringBuilder()
        val n = args.size

        // Time backport invocations — D8 routes into j$/time/*.
        out.append(TimeOps.t01(n)); out.append("\n")
        out.append(TimeOps.t02(n)); out.append("\n")
        out.append(TimeOps.t03(n)); out.append("\n")
        out.append(TimeOps.t04(n)); out.append("\n")
        out.append(TimeOps.t05(n)); out.append("\n")

        // Stream backport invocations — D8 routes into j$/util/stream/*.
        out.append(StreamOps.s01(n)); out.append("\n")
        out.append(StreamOps.s02(n)); out.append("\n")
        out.append(StreamOps.s03(n)); out.append("\n")
        out.append(StreamOps.s04(n)); out.append("\n")
        out.append(StreamOps.s05(n)); out.append("\n")

        // Optional invocations — may or may not route via j$ depending
        // on min-api; either way, exercises the API.
        out.append(OptionalOps.o01(n)); out.append("\n")
        out.append(OptionalOps.o02(n)); out.append("\n")

        // Outline-like callers — positive control for the ratchet.
        out.append(OutlineLike.n01(n)); out.append(OutlineLike.n02(n + 1)); out.append(OutlineLike.n03(n + 2))
        out.append(OutlineLike.n04(n)); out.append(OutlineLike.n05(n + 1)); out.append(OutlineLike.n06(n + 2))
        out.append(OutlineLike.n07(n)); out.append(OutlineLike.n08(n + 1)); out.append(OutlineLike.n09(n + 2))
        out.append(OutlineLike.n10(n)); out.append(OutlineLike.n11(n + 1)); out.append(OutlineLike.n12(n + 2))
        out.append(OutlineLike.n13(n)); out.append(OutlineLike.n14(n + 1)); out.append(OutlineLike.n15(n + 2))
        out.append(OutlineLike.n16(n)); out.append(OutlineLike.n17(n + 1)); out.append(OutlineLike.n18(n + 2))
        out.append(OutlineLike.n19(n)); out.append(OutlineLike.n20(n + 1)); out.append(OutlineLike.n21(n + 2))
        out.append(OutlineLike.n22(n)); out.append(OutlineLike.n23(n + 1)); out.append(OutlineLike.n24(n + 2))
        out.append(OutlineLike.n25(n)); out.append(OutlineLike.n26(n + 1)); out.append(OutlineLike.n27(n + 2))
        out.append(OutlineLike.n28(n)); out.append(OutlineLike.n29(n + 1)); out.append(OutlineLike.n30(n + 2))

        println(out.toString())
    }
}
