// javac-21 corpus seed: 50-arm String switch (medium-large).
// Probes whether the recognizer scales without per-arm cost blowup.
class T {
    static String f(String k) {
        switch (k) {
            case "k00": return "v00";
            case "k01": return "v01";
            case "k02": return "v02";
            case "k03": return "v03";
            case "k04": return "v04";
            case "k05": return "v05";
            case "k06": return "v06";
            case "k07": return "v07";
            case "k08": return "v08";
            case "k09": return "v09";
            case "k10": return "v10";
            case "k11": return "v11";
            case "k12": return "v12";
            case "k13": return "v13";
            case "k14": return "v14";
            case "k15": return "v15";
            case "k16": return "v16";
            case "k17": return "v17";
            case "k18": return "v18";
            case "k19": return "v19";
            case "k20": return "v20";
            case "k21": return "v21";
            case "k22": return "v22";
            case "k23": return "v23";
            case "k24": return "v24";
            case "k25": return "v25";
            case "k26": return "v26";
            case "k27": return "v27";
            case "k28": return "v28";
            case "k29": return "v29";
            case "k30": return "v30";
            case "k31": return "v31";
            case "k32": return "v32";
            case "k33": return "v33";
            case "k34": return "v34";
            case "k35": return "v35";
            case "k36": return "v36";
            case "k37": return "v37";
            case "k38": return "v38";
            case "k39": return "v39";
            case "k40": return "v40";
            case "k41": return "v41";
            case "k42": return "v42";
            case "k43": return "v43";
            case "k44": return "v44";
            case "k45": return "v45";
            case "k46": return "v46";
            case "k47": return "v47";
            case "k48": return "v48";
            case "k49": return "v49";
        }
        return "?";
    }
}
