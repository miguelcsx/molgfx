use super::{
    COLUMN_WORDS, GROUP_WORDS, KEYWORD_ALIASES, canonical_keyword, describe_query_finding,
    nearest_keywords,
};

#[test]
fn every_group_word_is_a_molframe_keyword() {
    for word in GROUP_WORDS {
        assert!(molframe::Query::compile(word).is_ok(), "{word}");
    }
}

#[test]
fn every_column_word_compiles_with_a_value() {
    for word in COLUMN_WORDS {
        let source = format!("{word} 1");
        assert!(molframe::Query::compile(&source).is_ok(), "{source}");
    }
}

#[test]
fn every_alias_points_at_a_canonical_word() {
    for (alias, canonical) in KEYWORD_ALIASES {
        assert!(
            GROUP_WORDS.contains(canonical) || COLUMN_WORDS.contains(canonical),
            "{alias} -> {canonical}"
        );
        assert!(
            !GROUP_WORDS.contains(alias) && !COLUMN_WORDS.contains(alias),
            "{alias}"
        );
    }
}

#[test]
fn a_plural_alias_resolves_and_a_typo_lists_near_words() {
    assert_eq!(canonical_keyword("Ligands"), Some("ligand"));
    assert_eq!(nearest_keywords("ligands").first(), Some(&"ligand"));
    assert!(nearest_keywords("liganf").contains(&"ligand"));
    assert!(nearest_keywords("zzzzzzzz").is_empty());
}

#[test]
fn an_unknown_keyword_finding_names_the_word() {
    let Err(findings) = molframe::Query::compile("ligandz") else {
        panic!("not a keyword");
    };
    let Some(first) = findings.first() else {
        panic!("a finding");
    };
    let (message, suggestion) = describe_query_finding(first);
    assert!(
        message.contains("'ligandz'") && message.contains("ligand"),
        "{message}"
    );
    assert_eq!(suggestion.as_deref(), Some("ligand"));
}
