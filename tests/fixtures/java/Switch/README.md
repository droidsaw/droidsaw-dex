# Switch

**Covers.** Dense integer switch with a `default` arm. d8 emits a `packed-switch` payload; exercises the decompiler's switch recovery and switch-payload handling.

**Status.** `compile_pass`. Recompiled program stdout matches `expected.txt` ("Mon\nWed\nweekend").
