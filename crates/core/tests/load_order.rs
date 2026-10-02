#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::collections::HashSet;
use wh3_core::{
    load_order::{
        self, DropReason, Resolution, Rule, RuleKey, RuleSource, is_rules_file_path,
        parse_rules_bytes, parse_rules_file,
    },
    localization::Language,
};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| format!("{name}.pack")).collect()
}

fn present(list: &[String]) -> HashSet<String> {
    list.iter().cloned().collect()
}

fn resolve_all(user: &[Rule], pack: &[Rule], mods: &[String]) -> Resolution {
    load_order::resolve(user, pack, &HashSet::new(), &HashSet::new(), &present(mods))
}

fn reasons(resolution: &Resolution) -> Vec<(String, String, DropReason)> {
    resolution
        .dropped
        .iter()
        .map(|(rule, reason)| (rule.before.clone(), rule.after.clone(), reason.clone()))
        .collect()
}

#[test]
fn rules_file_is_relative_to_its_pack_and_tolerant() {
    let text =
        "# header\r\n\r\nBEFORE\tb.pack\r\n  after    C  \r\nBefore data/sub\\d.pack\n#AFTER x\n";
    let (rules, warnings) = parse_rules_file("A.pack", text);
    assert!(warnings.is_empty());
    let source = RuleSource::Pack("A.pack".into());
    assert_eq!(
        rules,
        vec![
            Rule {
                before: "A.pack".into(),
                after: "b.pack".into(),
                subject: "A.pack".into(),
                source: source.clone()
            },
            Rule {
                before: "C.pack".into(),
                after: "A.pack".into(),
                subject: "A.pack".into(),
                source: source.clone()
            },
            Rule {
                before: "A.pack".into(),
                after: "d.pack".into(),
                subject: "A.pack".into(),
                source
            },
        ]
    );
}

#[test]
fn bad_rule_lines_are_skipped_with_bilingual_line_warnings() {
    let text = "BEFORE\nBEFORE two words\nSIDEWAYS b\nAFTER a.PACK\nBEFORE ok\nAFTER /\n";
    let (rules, warnings) = parse_rules_file("a", text);
    assert_eq!(rules, vec![Rule::pack("a.pack", "a.pack", "ok.pack")]);
    let english: Vec<&str> = warnings.iter().map(|w| w.text(Language::English)).collect();
    assert_eq!(english.len(), 5);
    for (warning, line) in english.iter().zip([1, 2, 3, 4, 6]) {
        assert!(
            warning.starts_with(&format!("a.pack: line {line}: ")),
            "{warning}"
        );
    }
    assert!(english[2].contains("\"SIDEWAYS\""));
    assert!(english[3].contains("itself"));
    assert!(
        warnings[0]
            .text(Language::Russian)
            .starts_with("a.pack: строка 1: ")
    );
}

#[test]
fn rules_file_bytes_accept_boms_and_report_bad_utf8() {
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("BEFORE b".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(
        parse_rules_bytes("a", &utf16).0,
        vec![Rule::pack("a", "a", "b")]
    );
    assert_eq!(
        parse_rules_bytes("a", b"\xEF\xBB\xBFAFTER b").0,
        vec![Rule::pack("a", "b", "a")]
    );

    let (rules, warnings) = parse_rules_bytes("a", b"BEFORE b\n\xFF\n");
    assert_eq!(rules, vec![Rule::pack("a", "a", "b")]);
    assert_eq!(
        warnings.len(),
        2,
        "one for the encoding, one for the garbled line"
    );
    assert!(is_rules_file_path(" WHMM/Load_Order.whmm "));
    assert!(!is_rules_file_path("whmm\\other.whmm"));
}

#[test]
fn rule_keys_match_the_original_format() {
    assert_eq!(
        Rule::pack("Src", "A.pack", "b").key().as_str(),
        "src.pack\ta.pack\tb.pack"
    );
    assert_eq!(
        Rule::user("A", "B", "A").key(),
        RuleKey::from_raw("\ta.pack\tb.pack")
    );
}

#[test]
fn user_rules_replace_pack_rules_about_the_same_pair() {
    let mods = names(&["a", "b", "c"]);
    let pack = [Rule::pack("a", "a", "b"), Rule::pack("c", "c", "b")];
    let user = [Rule::user("b", "a", "b"), Rule::user("C", "B", "c")];
    let resolution = resolve_all(&user, &pack, &mods);
    assert_eq!(resolution.accepted, user.to_vec());
    assert_eq!(
        reasons(&resolution),
        vec![
            (
                "a.pack".into(),
                "b.pack".into(),
                DropReason::OverriddenByUser
            ),
            (
                "c.pack".into(),
                "b.pack".into(),
                DropReason::OverriddenByUser
            ),
        ]
    );
}

#[test]
fn packs_disagreeing_about_a_pair_cancel_while_agreeing_ones_stay() {
    let mods = names(&["a", "b", "c", "d"]);
    let pack = [
        Rule::pack("a", "a", "b"),
        Rule::pack("b", "b", "a"),
        Rule::pack("c", "a", "b"),
        Rule::pack("c", "c", "d"),
        Rule::pack("d", "c", "d"),
    ];
    let resolution = resolve_all(&[], &pack, &mods);
    assert_eq!(resolution.accepted, vec![pack[3].clone(), pack[4].clone()]);
    let dropped: Vec<_> = resolution
        .dropped
        .iter()
        .map(|(_, reason)| reason)
        .collect();
    assert_eq!(dropped, vec![&DropReason::Contradiction; 3]);
}

#[test]
fn cycles_drop_the_same_rule_whatever_the_input_order() {
    let mods = names(&["a", "b", "c"]);
    let pack = [
        Rule::pack("a", "a", "b"),
        Rule::pack("b", "b", "c"),
        Rule::pack("c", "c", "a"),
    ];
    let mut reversed = pack.to_vec();
    reversed.reverse();
    for input in [pack.to_vec(), reversed] {
        let resolution = resolve_all(&[], &input, &mods);
        assert_eq!(resolution.accepted, pack[..2].to_vec());
        assert_eq!(
            resolution.dropped,
            vec![(pack[2].clone(), DropReason::Cycle)]
        );
    }

    // User rules are accepted first, so the pack rule closing the loop gives way instead.
    let user = [Rule::user("c", "a", "c")];
    let resolution = resolve_all(&user, &pack[..2], &mods);
    assert_eq!(resolution.accepted, vec![user[0].clone(), pack[0].clone()]);
    assert_eq!(
        resolution.dropped,
        vec![(pack[1].clone(), DropReason::Cycle)]
    );
}

#[test]
fn missing_invalid_and_disabled_rules_are_dropped_before_conflicts() {
    let mods = names(&["a", "b", "c"]);
    let pack = [
        Rule::pack("a", "a", "b"),
        Rule::pack("b", "b", "a"),
        Rule::pack("c", "c", "a"),
        Rule::pack("a", "a", "zz"),
    ];
    let user = [Rule::user("a", "A.PACK", "a")];
    let disabled_rules = HashSet::from([pack[1].key()]);
    let disabled_packs = HashSet::from(["C".to_owned()]);
    let resolution = load_order::resolve(
        &user,
        &pack,
        &disabled_rules,
        &disabled_packs,
        &present(&mods),
    );
    // Muting b's rule rescues a's, which would otherwise have been cancelled as a contradiction.
    assert_eq!(resolution.accepted, vec![pack[0].clone()]);
    assert_eq!(
        reasons(&resolution),
        vec![
            ("b.pack".into(), "a.pack".into(), DropReason::Disabled),
            ("c.pack".into(), "a.pack".into(), DropReason::DisabledPack),
            ("a.pack".into(), "A.PACK".into(), DropReason::Invalid),
            (
                "a.pack".into(),
                "zz.pack".into(),
                DropReason::MissingPack("zz.pack".into())
            ),
        ]
    );
}
