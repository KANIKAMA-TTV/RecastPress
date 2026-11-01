use recastpress_db::{
    TermMetaRow, TermRelationshipKey, TermRelationshipRow, TermRow, TermTaxonomyRow,
};

#[test]
fn taxonomy_debug_omits_editing_text_and_metadata() {
    let secret = b"fixture-private-text".to_vec();
    let term = TermRow {
        term_id: 1,
        name: secret.clone(),
        slug: secret.clone(),
        term_group: -1,
    };
    let taxonomy = TermTaxonomyRow {
        term_taxonomy_id: 2,
        term_id: 1,
        taxonomy: secret.clone(),
        description: secret.clone(),
        parent: 0,
        count: -1,
    };
    let meta = TermMetaRow {
        meta_id: 3,
        term_id: 1,
        meta_key: Some(secret.clone()),
        meta_value: Some(secret),
    };
    let relationship = TermRelationshipRow {
        key: TermRelationshipKey {
            object_id: 4,
            term_taxonomy_id: 2,
        },
        term_order: -1,
    };
    for debug in [
        format!("{term:?}"),
        format!("{taxonomy:?}"),
        format!("{meta:?}"),
        format!("{relationship:?}"),
    ] {
        assert!(!debug.contains("fixture-private-text"));
        assert!(!debug.contains("102, 105, 120")); // Vec<u8>'s derived Debug representation.
    }
}
