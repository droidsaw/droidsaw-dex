// PoC subject for the DEX id-section overlap fixture. Default package
// (matches the tests/fixtures/java convention). Three members make the
// method-table corruption legible after the adversarial mutation:
//   - `marker`   : a static int FIELD. Its field_id_item is the same 8-byte
//                  shape as a method_id_item, so aliasing method_ids onto
//                  field_ids decodes the field row AS a method.
//   - `harmless` : a benign method.
//   - `sensitive`: references Runtime.exec — the surface an analyst cares about.
// Built by build.sh into the benign base.dex; the one-field mutation
// (method_ids_off := field_ids_off) + Adler-32 reseal produces overlap.dex.
public class SectionOverlap {
    public static int marker = 0;

    public static String harmless() {
        return "ok";
    }

    public static void sensitive() throws Exception {
        Runtime.getRuntime().exec(new String[] { "/system/bin/id" });
    }

    public static String run() throws Exception {
        harmless();
        return "ran";
    }
}
