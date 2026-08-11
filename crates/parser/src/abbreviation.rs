//! 期刊名称缩写（依据 Abbr.pdf —— IEEE "Useful Abbreviations in References" 词表）
//!
//! 提供将期刊全称转换为 IEEE 风格缩写的能力。词表仅作为代码常量存放，
//! 不引入任何数据库表；生成的缩写写入 `publications.abbreviation` 字段。

use std::collections::HashMap;
use std::sync::LazyLock;

/// 缩写时丢弃的停用词（介词/冠词等，不参与缩写、也不原样保留）
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "for", "from", "in", "of", "on", "the", "to", "with",
];

/// Abbr.pdf 提取的词 → 缩写对应表。
///
/// 条目包含括号（如 `Behavior(al)`）或逗号（如 `Africa, African`）表示的复合形式，
/// 构建索引时自动展开为各个词形。
#[rustfmt::skip]
const JOURNAL_ABBREVIATIONS: &[(&str, &str)] = &[
    ("Acoustics", "Acoust."),
    ("Active", "Act."),
    ("Abstracts", "Abstr."),
    ("Administration", "Admin."),
    ("Academy", "Acad."),
    ("Administrative", "Administ."),
    ("Accelerator", "Accel."),
    ("Advanced", "Adv."),
    ("Aeronautics", "Aeronaut."),
    ("Business", "Bus."),
    ("Aerospace", "Aerosp."),
    ("Affective", "Affect."),
    ("Africa, African", "Afr."),
    ("Canadian", "Can."),
    ("Aircraft", "Aircr."),
    ("Ceramic", "Ceram."),
    ("Algebraic", "Algebr."),
    ("Chemical", "Chem."),
    ("American", "Amer."),
    ("Chinese", "Chin."),
    ("Analysis", "Anal."),
    ("Climatology", "Climatol."),
    ("Annals", "Ann."),
    ("Clinical", "Clin."),
    ("Annual", "Annu."),
    ("Cognitive", "Cogn."),
    ("Apparatus", "App."),
    ("Colloquium", "Colloq."),
    ("Applications", "Appl."),
    ("Communications", "Commun."),
    ("Applied", "Appl."),
    ("Compatibility", "Compat."),
    ("Approximate", "Approx."),
    ("Component(s)", "Compon."),
    ("Architecture", "Archit."),
    ("Computational", "Comput."),
    ("Archive(s)", "Arch."),
    ("Computer(s)", "Comput."),
    ("Artificial", "Artif."),
    ("Computing", "Comput."),
    ("Assembly", "Assem."),
    ("Condensed", "Condens."),
    ("Association", "Assoc."),
    ("Conference", "Conf."),
    ("Astronomy", "Astron."),
    ("Congress", "Congr."),
    ("Astronautics", "Astronaut."),
    ("Consumer", "Consum."),
    ("Astrophysics", "Astrophys."),
    ("Conversion", "Convers."),
    ("Atmosphere", "Atmos."),
    ("Convention", "Conv."),
    ("Atomic, Atoms", "At."),
    ("Correspondence", "Corresp."),
    ("Australasian", "Australas."),
    ("Critical", "Crit."),
    ("Australia", "Aust."),
    ("Crystal", "Cryst."),
    ("Automatic", "Autom."),
    ("Crystallography", "Crystallogr."),
    ("Automation", "Automat."),
    ("Cybernetics", "Cybern."),
    ("Automotive", "Automot."),
    ("Autonomous", "Auton."),
    ("Decision", "Decis."),
    ("Delivery", "Del."),
    ("Behavior(al)", "Behav."),
    ("Department", "Dept."),
    ("Belgian", "Belg."),
    ("Design", "Des."),
    ("Biochemical", "Biochem."),
    ("Detector", "Detect."),
    ("Bioinformatics", "Bioinf."),
    ("Development(al)", "Develop."),
    ("Biology, Biological", "Biol."),
    ("Differential", "Differ."),
    ("Biomedical", "Biomed."),
    ("Digest", "Dig."),
    ("Biophysics", "Biophys."),
    ("Digital", "Digit."),
    ("British", "Brit."),
    ("Disclosure", "Discl."),
    ("Broadcasting", "Broadcast."),
    ("Discussions", "Discuss."),
    ("Bulletin", "Bull."),
    ("Dissertations", "Diss."),
    ("Bureau", "Bur."),
    ("Distributed", "Distrib."),
    ("Dynamics", "Dyn."),
    ("Harmonic(s)", "Harmon."),
    ("Earthquake", "Earthq."),
    ("History", "Hist."),
    ("Economic(s)", "Econ."),
    ("Horizon", "Horiz."),
    ("Edition", "Ed."),
    ("Hungary, Hungarian", "Hung."),
    ("Education", "Educ."),
    ("Hydraulics", "Hydraul."),
    ("Electrical", "Elect."),
    ("Hydrology", "Hydrol."),
    ("Electrification", "Electrific."),
    ("Electromagnetic", "Electromagn."),
    ("Electroacoustic", "Electroacoust."),
    ("Illuminating", "Illum."),
    ("Electronic", "Electron."),
    ("Imaging", "Imag."),
    ("Emerging", "Emerg."),
    ("Industrial", "Ind."),
    ("Engineering", "Eng."),
    ("Information", "Inf."),
    ("Environment", "Environ."),
    ("Informatics", "Inform."),
    ("Equations", "Equ."),
    ("Innovation", "Innov."),
    ("Equipment", "Equip."),
    ("Institute", "Inst."),
    ("Ergonomics", "Ergonom."),
    ("Instrument", "Instrum."),
    ("European", "Eur."),
    ("Instrumentation", "Instrum."),
    ("Evaluation", "Eval."),
    ("Insulation", "Insul."),
    ("Evolutionary", "Evol."),
    ("Integrated", "Integr."),
    ("Exhibition", "Exhib."),
    ("Intelligence", "Intell."),
    ("Experimental", "Exp."),
    ("Intelligent", "Intell."),
    ("Exploratory", "Explor."),
    ("Interactions", "Interact."),
    ("Exposition", "Expo."),
    ("International", "Int."),
    ("Express", "Exp."),
    ("Isotopes", "Isot."),
    ("Israel", "Isr."),
    ("Fabrication", "Fabr."),
    ("Faculty", "Fac."),
    ("Japan", "Jpn."),
    ("Ferroelectrics", "Ferroelect."),
    ("Journal", "J."),
    ("Francais, French", "Fr."),
    ("Frequency", "Freq."),
    ("Foundation", "Found."),
    ("Knowledge", "Knowl."),
    ("Fundamental", "Fundam."),
    ("Laboratory(ies)", "Lab."),
    ("Generation", "Gener."),
    ("Language", "Lang."),
    ("Geology", "Geol."),
    ("Learning", "Learn."),
    ("Geophysics", "Geophys."),
    ("Letter(s)", "Lett."),
    ("Geoscience", "Geosci."),
    ("Lightwave", "Lightw."),
    ("Graphics", "Graph."),
    ("Logic, Logical", "Log."),
    ("Guidance", "Guid."),
    ("Luminescence", "Lumin."),
    ("Observations", "Observ."),
    ("Oceanic", "Ocean."),
    ("Machine", "Mach."),
    ("Oceanography", "Oceanogr."),
    ("Magazine", "Mag."),
    ("Occupation", "Occupat."),
    ("Magnetics", "Magn."),
    ("Operational", "Oper."),
    ("Management", "Manage."),
    ("Optical", "Opt."),
    ("Managing", "Manag."),
    ("Optics", "Opt."),
    ("Manufacturing", "Manuf."),
    ("Optimization", "Optim."),
    ("Marine", "Mar."),
    ("Organization", "Org."),
    ("Material", "Mater."),
    ("Mathematical", "Math."),
    ("Mathematics", "Math."),
    ("Packaging", "Packag."),
    ("Measurement", "Meas."),
    ("Particle", "Part."),
    ("Mechanical", "Mech."),
    ("Patent", "Pat."),
    ("Medical, Medicine", "Med."),
    ("Performance", "Perform."),
    ("Metals", "Met."),
    ("Personal", "Pers."),
    ("Metallurgy", "Metall."),
    ("Philosophical", "Philos."),
    ("Meteorology", "Meteorol."),
    ("Photonics", "Photon."),
    ("Metropolitan", "Metrop."),
    ("Photovoltaics", "Photovolt."),
    ("Mexican, Mexico", "Mex."),
    ("Physics", "Phys."),
    ("Microelectromechanical", "Microelectromech."),
    ("Physiology", "Physiol."),
    ("Microgravity", "Microgr."),
    ("Planetary", "Planet."),
    ("Microscopy", "Microsc."),
    ("Pneumatics", "Pneum."),
    ("Microwave(s)", "Microw."),
    ("Pollution", "Pollut."),
    ("Military", "Mil."),
    ("Polymer", "Polym."),
    ("Modeling", "Model."),
    ("Polytechnic", "Polytech."),
    ("Molecular", "Mol."),
    ("Practice", "Pract."),
    ("Monitoring", "Monit."),
    ("Precision", "Precis."),
    ("Multiphysics", "Multiphys."),
    ("Principles", "Princ."),
    ("Proceedings", "Proc."),
    ("Processing", "Process."),
    ("Nanobioscience", "Nanobiosci."),
    ("Production", "Prod."),
    ("Nanotechnology", "Nanotechnol."),
    ("Productivity", "Productiv."),
    ("National", "Nat."),
    ("Programmable", "Program."),
    ("Naval", "Nav."),
    ("Programming", "Program."),
    ("Navigation", "Navig."),
    ("Progress", "Prog."),
    ("Network, Networking, Networks", "Netw."),
    ("Propagation", "Propag."),
    ("Newsletter", "Newslett."),
    ("Psychology", "Psychol."),
    ("Nondestructive", "Nondestruct."),
    ("Nuclear", "Nucl."),
    ("Numerical", "Numer."),
    ("Quality", "Qual."),
    ("Quarterly", "Quart."),
    ("Structure", "Struct."),
    ("Radiation", "Radiat."),
    ("Studies", "Stud."),
    ("Radiology", "Radiol."),
    ("Superconductivity", "Supercond."),
    ("Reactor", "React."),
    ("Supplement", "Suppl."),
    ("Receivers", "Receiv."),
    ("Surface", "Surf."),
    ("Recognition", "Recognit."),
    ("Survey", "Surv."),
    ("Record", "Rec."),
    ("Sustainable", "Sustain."),
    ("Rehabilitation", "Rehabil."),
    ("Symposium", "Symp."),
    ("Reliability", "Rel."),
    ("Systems", "Syst."),
    ("Report", "Rep."),
    ("Research", "Res."),
    ("Resonance", "Reson."),
    ("Technical", "Tech."),
    ("Resources", "Resour."),
    ("Techniques", "Techn."),
    ("Review", "Rev."),
    ("Technology", "Technol."),
    ("Robotics", "Robot."),
    ("Telecommunications", "Telecommun."),
    ("Royal", "Roy."),
    ("Television", "Telev."),
    ("Temperature", "Temp."),
    ("Terrestrial", "Terr."),
    ("Safety", "Saf."),
    ("Theoretical", "Theor."),
    ("Satellite", "Satell."),
    ("Transactions", "Trans."),
    ("Scandinavian", "Scand."),
    ("Translation", "Transl."),
    ("Science, Sciences", "Sci."),
    ("Transmission", "Transmiss."),
    ("Section", "Sect."),
    ("Transportation", "Transp."),
    ("Security", "Secur."),
    ("Tutorials", "Tut."),
    ("Seismology", "Seismol."),
    ("Selected", "Sel."),
    ("Semiconductor", "Semicond."),
    ("Ultrasonic", "Ultrason."),
    ("Sensing", "Sens."),
    ("University", "Univ."),
    ("Series", "Ser."),
    ("Simulation", "Simul."),
    ("Singapore", "Singap."),
    ("Vacuum", "Vac."),
    ("Sistema", "Sist."),
    ("Vehicular", "Veh."),
    ("Society", "Soc."),
    ("Vibration", "Vib."),
    ("Sociological", "Sociol."),
    ("Vision", "Vis."),
    ("Software", "Softw."),
    ("Visual", "Vis."),
    ("Solar", "Sol."),
    ("Soviet", "Sov."),
    ("Spectroscopy", "Spectrosc."),
    ("Welding", "Weld."),
    ("Spectrum", "Spectr."),
    ("Working", "Work."),
    ("Speculations", "Specul."),
    ("Statistics", "Statist."),
];

/// 将带括号/逗号的复合词展开为各个词形，返回全小写列表。
///
/// - `Behavior(al)` → `["behavior", "behavioral"]`
/// - `Laboratory(ies)` → `["laboratory", "laboratories"]`
/// - `Africa, African` → `["africa", "african"]`
fn expand_variants(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in word.split(',') {
        let part = part.trim();
        if let Some(open) = part.find('(') {
            let base = part[..open].trim();
            let inside = &part[open + 1..part.len() - 1];
            out.push(base.to_lowercase());
            match inside {
                "s" => out.push(format!("{base}s").to_lowercase()),
                "al" => out.push(format!("{base}al").to_lowercase()),
                "ies" => {
                    let stem = base.trim_end_matches('y');
                    out.push(format!("{stem}ies").to_lowercase());
                }
                _ => {}
            }
        } else {
            out.push(part.to_lowercase());
        }
    }
    out
}

static ABBR_MAP: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for &(word, abbr) in JOURNAL_ABBREVIATIONS {
        for variant in expand_variants(word) {
            map.insert(variant, abbr);
        }
    }
    map
});

/// 将期刊名称缩写为 IEEE 风格。
///
/// 规则：
/// - 按空白分词，丢弃停用词（on/of/the/and 等）；
/// - 表内词替换为缩写（含末尾句点），表外词保留原文；
/// - 词首尾的标点（如句点、逗号）不参与匹配。
pub fn abbreviate_journal_name(name: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for token in name.split_whitespace() {
        let lower = token.to_lowercase();
        let key = lower.trim_matches(|c: char| !c.is_alphanumeric());
        if STOP_WORDS.contains(&key) {
            continue;
        }
        if let Some(abbr) = ABBR_MAP.get(key) {
            out.push((*abbr).to_string());
        } else {
            out.push(token.to_string());
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abbreviate_basic() {
        assert_eq!(
            abbreviate_journal_name("IEEE Transactions on Information Theory"),
            "IEEE Trans. Inf. Theory"
        );
        assert_eq!(
            abbreviate_journal_name("Journal of Applied Physics"),
            "J. Appl. Phys."
        );
        assert_eq!(
            abbreviate_journal_name("Proceedings of the National Academy of Sciences"),
            "Proc. Nat. Acad. Sci."
        );
    }

    #[test]
    fn test_abbreviate_plural_variants() {
        assert_eq!(
            abbreviate_journal_name("Communications Letters"),
            "Commun. Lett."
        );
        assert_eq!(abbreviate_journal_name("Computer Systems"), "Comput. Syst.");
        assert_eq!(abbreviate_journal_name("Behavioral Science"), "Behav. Sci.");
        assert_eq!(abbreviate_journal_name("Laboratories"), "Lab.");
    }

    #[test]
    fn test_abbreviate_stop_words_dropped() {
        assert_eq!(
            abbreviate_journal_name("IEEE Transactions on Signal Processing"),
            "IEEE Trans. Signal Process."
        );
        assert_eq!(
            abbreviate_journal_name("Annals of the Association"),
            "Ann. Assoc."
        );
    }

    #[test]
    fn test_abbreviate_keeps_unknown() {
        assert_eq!(abbreviate_journal_name("Neural Networks"), "Neural Netw.");
        assert_eq!(
            abbreviate_journal_name("Pattern Recognition"),
            "Pattern Recognit."
        );
    }

    #[test]
    fn test_abbreviate_empty_and_short() {
        assert_eq!(abbreviate_journal_name(""), "");
        assert_eq!(abbreviate_journal_name("IEEE"), "IEEE");
    }
}
