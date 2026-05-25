// Sealed-OBJECT `when` at 130-arm scale. MYB.A00 acceptance-probe scale
// (Brief §6, advisory probe). Audit confirmed kotlinc-1.9.22 emits the
// same linear `Intrinsics.areEqual` chain at this size — no separate
// threshold-driven lowering observed. Inclusion is conservative coverage.

sealed class Tag {
    object T1 : Tag()
    object T2 : Tag()
    object T3 : Tag()
    object T4 : Tag()
    object T5 : Tag()
    object T6 : Tag()
    object T7 : Tag()
    object T8 : Tag()
    object T9 : Tag()
    object T10 : Tag()
    object T11 : Tag()
    object T12 : Tag()
    object T13 : Tag()
    object T14 : Tag()
    object T15 : Tag()
    object T16 : Tag()
    object T17 : Tag()
    object T18 : Tag()
    object T19 : Tag()
    object T20 : Tag()
    object T21 : Tag()
    object T22 : Tag()
    object T23 : Tag()
    object T24 : Tag()
    object T25 : Tag()
    object T26 : Tag()
    object T27 : Tag()
    object T28 : Tag()
    object T29 : Tag()
    object T30 : Tag()
    object T31 : Tag()
    object T32 : Tag()
    object T33 : Tag()
    object T34 : Tag()
    object T35 : Tag()
    object T36 : Tag()
    object T37 : Tag()
    object T38 : Tag()
    object T39 : Tag()
    object T40 : Tag()
    object T41 : Tag()
    object T42 : Tag()
    object T43 : Tag()
    object T44 : Tag()
    object T45 : Tag()
    object T46 : Tag()
    object T47 : Tag()
    object T48 : Tag()
    object T49 : Tag()
    object T50 : Tag()
    object T51 : Tag()
    object T52 : Tag()
    object T53 : Tag()
    object T54 : Tag()
    object T55 : Tag()
    object T56 : Tag()
    object T57 : Tag()
    object T58 : Tag()
    object T59 : Tag()
    object T60 : Tag()
    object T61 : Tag()
    object T62 : Tag()
    object T63 : Tag()
    object T64 : Tag()
    object T65 : Tag()
    object T66 : Tag()
    object T67 : Tag()
    object T68 : Tag()
    object T69 : Tag()
    object T70 : Tag()
    object T71 : Tag()
    object T72 : Tag()
    object T73 : Tag()
    object T74 : Tag()
    object T75 : Tag()
    object T76 : Tag()
    object T77 : Tag()
    object T78 : Tag()
    object T79 : Tag()
    object T80 : Tag()
    object T81 : Tag()
    object T82 : Tag()
    object T83 : Tag()
    object T84 : Tag()
    object T85 : Tag()
    object T86 : Tag()
    object T87 : Tag()
    object T88 : Tag()
    object T89 : Tag()
    object T90 : Tag()
    object T91 : Tag()
    object T92 : Tag()
    object T93 : Tag()
    object T94 : Tag()
    object T95 : Tag()
    object T96 : Tag()
    object T97 : Tag()
    object T98 : Tag()
    object T99 : Tag()
    object T100 : Tag()
    object T101 : Tag()
    object T102 : Tag()
    object T103 : Tag()
    object T104 : Tag()
    object T105 : Tag()
    object T106 : Tag()
    object T107 : Tag()
    object T108 : Tag()
    object T109 : Tag()
    object T110 : Tag()
    object T111 : Tag()
    object T112 : Tag()
    object T113 : Tag()
    object T114 : Tag()
    object T115 : Tag()
    object T116 : Tag()
    object T117 : Tag()
    object T118 : Tag()
    object T119 : Tag()
    object T120 : Tag()
    object T121 : Tag()
    object T122 : Tag()
    object T123 : Tag()
    object T124 : Tag()
    object T125 : Tag()
    object T126 : Tag()
    object T127 : Tag()
    object T128 : Tag()
    object T129 : Tag()
    object T130 : Tag()
}

fun describe(t: Tag): String = when (t) {
    Tag.T1 -> "t1"
    Tag.T2 -> "t2"
    Tag.T3 -> "t3"
    Tag.T4 -> "t4"
    Tag.T5 -> "t5"
    Tag.T6 -> "t6"
    Tag.T7 -> "t7"
    Tag.T8 -> "t8"
    Tag.T9 -> "t9"
    Tag.T10 -> "t10"
    Tag.T11 -> "t11"
    Tag.T12 -> "t12"
    Tag.T13 -> "t13"
    Tag.T14 -> "t14"
    Tag.T15 -> "t15"
    Tag.T16 -> "t16"
    Tag.T17 -> "t17"
    Tag.T18 -> "t18"
    Tag.T19 -> "t19"
    Tag.T20 -> "t20"
    Tag.T21 -> "t21"
    Tag.T22 -> "t22"
    Tag.T23 -> "t23"
    Tag.T24 -> "t24"
    Tag.T25 -> "t25"
    Tag.T26 -> "t26"
    Tag.T27 -> "t27"
    Tag.T28 -> "t28"
    Tag.T29 -> "t29"
    Tag.T30 -> "t30"
    Tag.T31 -> "t31"
    Tag.T32 -> "t32"
    Tag.T33 -> "t33"
    Tag.T34 -> "t34"
    Tag.T35 -> "t35"
    Tag.T36 -> "t36"
    Tag.T37 -> "t37"
    Tag.T38 -> "t38"
    Tag.T39 -> "t39"
    Tag.T40 -> "t40"
    Tag.T41 -> "t41"
    Tag.T42 -> "t42"
    Tag.T43 -> "t43"
    Tag.T44 -> "t44"
    Tag.T45 -> "t45"
    Tag.T46 -> "t46"
    Tag.T47 -> "t47"
    Tag.T48 -> "t48"
    Tag.T49 -> "t49"
    Tag.T50 -> "t50"
    Tag.T51 -> "t51"
    Tag.T52 -> "t52"
    Tag.T53 -> "t53"
    Tag.T54 -> "t54"
    Tag.T55 -> "t55"
    Tag.T56 -> "t56"
    Tag.T57 -> "t57"
    Tag.T58 -> "t58"
    Tag.T59 -> "t59"
    Tag.T60 -> "t60"
    Tag.T61 -> "t61"
    Tag.T62 -> "t62"
    Tag.T63 -> "t63"
    Tag.T64 -> "t64"
    Tag.T65 -> "t65"
    Tag.T66 -> "t66"
    Tag.T67 -> "t67"
    Tag.T68 -> "t68"
    Tag.T69 -> "t69"
    Tag.T70 -> "t70"
    Tag.T71 -> "t71"
    Tag.T72 -> "t72"
    Tag.T73 -> "t73"
    Tag.T74 -> "t74"
    Tag.T75 -> "t75"
    Tag.T76 -> "t76"
    Tag.T77 -> "t77"
    Tag.T78 -> "t78"
    Tag.T79 -> "t79"
    Tag.T80 -> "t80"
    Tag.T81 -> "t81"
    Tag.T82 -> "t82"
    Tag.T83 -> "t83"
    Tag.T84 -> "t84"
    Tag.T85 -> "t85"
    Tag.T86 -> "t86"
    Tag.T87 -> "t87"
    Tag.T88 -> "t88"
    Tag.T89 -> "t89"
    Tag.T90 -> "t90"
    Tag.T91 -> "t91"
    Tag.T92 -> "t92"
    Tag.T93 -> "t93"
    Tag.T94 -> "t94"
    Tag.T95 -> "t95"
    Tag.T96 -> "t96"
    Tag.T97 -> "t97"
    Tag.T98 -> "t98"
    Tag.T99 -> "t99"
    Tag.T100 -> "t100"
    Tag.T101 -> "t101"
    Tag.T102 -> "t102"
    Tag.T103 -> "t103"
    Tag.T104 -> "t104"
    Tag.T105 -> "t105"
    Tag.T106 -> "t106"
    Tag.T107 -> "t107"
    Tag.T108 -> "t108"
    Tag.T109 -> "t109"
    Tag.T110 -> "t110"
    Tag.T111 -> "t111"
    Tag.T112 -> "t112"
    Tag.T113 -> "t113"
    Tag.T114 -> "t114"
    Tag.T115 -> "t115"
    Tag.T116 -> "t116"
    Tag.T117 -> "t117"
    Tag.T118 -> "t118"
    Tag.T119 -> "t119"
    Tag.T120 -> "t120"
    Tag.T121 -> "t121"
    Tag.T122 -> "t122"
    Tag.T123 -> "t123"
    Tag.T124 -> "t124"
    Tag.T125 -> "t125"
    Tag.T126 -> "t126"
    Tag.T127 -> "t127"
    Tag.T128 -> "t128"
    Tag.T129 -> "t129"
    Tag.T130 -> "t130"
}
