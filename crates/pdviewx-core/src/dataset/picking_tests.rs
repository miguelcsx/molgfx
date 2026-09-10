use super::*;

fn span(first: u64, count: u32) -> ChunkSpan {
    match ChunkSpan::new(LogicalRow::new(first), count) {
        Ok(value) => value,
        Err(error) => panic!("valid test span: {error}"),
    }
}

fn resolver(capacity: u32) -> PagedPickResolver {
    match PagedPickResolver::new(capacity) {
        Ok(value) => value,
        Err(error) => panic!("valid test resolver: {error}"),
    }
}

fn request_ready(
    resolver: &mut PagedPickResolver,
    descriptor: PickPageDescriptor,
) -> PickPageTicket {
    let ticket = match resolver.request(descriptor) {
        Ok(value) => value,
        Err(error) => panic!("page request succeeds: {error}"),
    };
    if let Err(error) = resolver.complete(ticket) {
        panic!("page completion succeeds: {error}");
    }
    ticket
}

fn resolve(resolver: &PagedPickResolver, ticket: PickPageTicket, row: u32) -> GlobalPickIdentity {
    let token = match resolver.token(ticket, LocalRow::new(row)) {
        Ok(value) => value,
        Err(error) => panic!("token creation succeeds: {error}"),
    };
    match resolver.resolve(PickReadback::new(token, ticket.generation())) {
        Ok(value) => value,
        Err(error) => panic!("pick resolution succeeds: {error}"),
    }
}

#[test]
fn compact_token_resolves_ids_and_rows_above_u32() {
    let high = u64::from(u32::MAX) + 9_000;
    let descriptor = PickPageDescriptor::new(
        DatasetId::new(high + 1),
        ChunkId::new(high + 2),
        span(high + 3, 8),
        EntityKind::Atom,
    );
    let mut resolver = resolver(2);
    let ticket = request_ready(&mut resolver, descriptor);
    let identity = resolve(&resolver, ticket, 7);

    assert_eq!(identity.dataset(), DatasetId::new(high + 1));
    assert_eq!(identity.chunk(), ChunkId::new(high + 2));
    assert_eq!(identity.row(), LogicalRow::new(high + 10));
    assert_eq!(identity.kind(), EntityKind::Atom);
    assert_eq!(core::mem::size_of::<GpuPickToken>(), 8);
}

#[test]
fn resolver_storage_is_fixed_by_resident_capacity() {
    let mut resolver = resolver(2);
    assert_eq!(resolver.capacity(), 2);
    let first = PickPageDescriptor::new(
        DatasetId::new(u64::MAX - 10),
        ChunkId::new(u64::MAX - 9),
        span(u64::MAX - 8, 2),
        EntityKind::Mesh,
    );
    let second = PickPageDescriptor::new(
        DatasetId::new(2),
        ChunkId::new(3),
        span(0, u32::MAX),
        EntityKind::Bond,
    );
    let _first = request_ready(&mut resolver, first);
    let _second = request_ready(&mut resolver, second);

    assert_eq!(resolver.resident_len(), 2);
    assert_eq!(resolver.request(first), Err(PickingError::DuplicatePage));
    let third = PickPageDescriptor::new(
        DatasetId::new(4),
        ChunkId::new(5),
        span(0, 1),
        EntityKind::Label,
    );
    assert_eq!(resolver.request(third), Err(PickingError::WorkingSetFull));
}

#[test]
fn one_chunk_namespace_cannot_alias_a_second_logical_span() {
    let mut resolver = resolver(2);
    let first = PickPageDescriptor::new(
        DatasetId::new(10),
        ChunkId::new(20),
        span(30, 2),
        EntityKind::Guide,
    );
    let alias = PickPageDescriptor::new(
        DatasetId::new(10),
        ChunkId::new(20),
        span(90, 2),
        EntityKind::Guide,
    );
    let _ticket = request_ready(&mut resolver, first);
    assert_eq!(resolver.request(alias), Err(PickingError::DuplicatePage));
}

#[test]
fn recycled_pages_reject_stale_tokens_and_completions() {
    let descriptor = PickPageDescriptor::new(
        DatasetId::new(1),
        ChunkId::new(10),
        span(100, 2),
        EntityKind::Atom,
    );
    let replacement = PickPageDescriptor::new(
        DatasetId::new(2),
        ChunkId::new(20),
        span(200, 2),
        EntityKind::Bond,
    );
    let mut resolver = resolver(1);
    let old = request_ready(&mut resolver, descriptor);
    let old_token = match resolver.token(old, LocalRow::new(1)) {
        Ok(value) => value,
        Err(error) => panic!("old token exists: {error}"),
    };
    if let Err(error) = resolver.release(old) {
        panic!("old page releases: {error}");
    }
    let new = match resolver.request(replacement) {
        Ok(value) => value,
        Err(error) => panic!("replacement request succeeds: {error}"),
    };

    assert_ne!(old.generation(), new.generation());
    assert_eq!(resolver.complete(old), Err(PickingError::StaleGeneration));
    if let Err(error) = resolver.complete(new) {
        panic!("replacement completion succeeds: {error}");
    }
    assert_eq!(
        resolver.resolve(PickReadback::new(old_token, old.generation())),
        Err(PickingError::StaleGeneration)
    );
    assert_eq!(resolve(&resolver, new, 1).row(), LogicalRow::new(201));
}

#[test]
fn guides_and_interactions_have_distinct_global_provenance() {
    let dataset = DatasetId::new(77);
    let chunk = ChunkId::new(88);
    let rows = span(u64::from(u32::MAX) + 1, 1);
    let mut resolver = resolver(2);
    let interaction = request_ready(
        &mut resolver,
        PickPageDescriptor::new(dataset, chunk, rows, EntityKind::Edge),
    );
    let guide = request_ready(
        &mut resolver,
        PickPageDescriptor::new(dataset, chunk, rows, EntityKind::Guide),
    );

    let interaction_identity = resolve(&resolver, interaction, 0);
    let guide_identity = resolve(&resolver, guide, 0);
    assert_eq!(interaction_identity.row(), guide_identity.row());
    assert_eq!(interaction_identity.dataset(), guide_identity.dataset());
    assert_ne!(interaction_identity, guide_identity);
    assert_eq!(interaction_identity.kind(), EntityKind::Edge);
    assert_eq!(guide_identity.kind(), EntityKind::Guide);
}

#[test]
fn every_entity_kind_is_an_independent_namespace() {
    let kinds = [
        EntityKind::Atom,
        EntityKind::Bond,
        EntityKind::Edge,
        EntityKind::Label,
        EntityKind::Primitive,
        EntityKind::Mesh,
        EntityKind::LigandPoseBatch,
        EntityKind::Guide,
        EntityKind::DynamicBond,
    ];
    let capacity = match u32::try_from(kinds.len()) {
        Ok(value) => value,
        Err(error) => panic!("entity kind count fits u32: {error}"),
    };
    let mut resolver = resolver(capacity);
    let mut identities = Vec::new();
    for kind in kinds {
        let ticket = request_ready(
            &mut resolver,
            PickPageDescriptor::new(DatasetId::new(1), ChunkId::new(2), span(3, 1), kind),
        );
        identities.push(resolve(&resolver, ticket, 0));
    }
    for (index, identity) in identities.iter().enumerate() {
        assert!(
            identities[index + 1..]
                .iter()
                .all(|other| other != identity)
        );
    }
}

#[test]
fn invalid_local_rows_and_empty_tokens_return_typed_errors() {
    let mut resolver = resolver(1);
    let ticket = request_ready(
        &mut resolver,
        PickPageDescriptor::new(
            DatasetId::new(1),
            ChunkId::new(2),
            span(3, 2),
            EntityKind::Primitive,
        ),
    );
    assert!(matches!(
        resolver.token(ticket, LocalRow::new(2)),
        Err(PickingError::LocalRowOutsidePage {
            page: 0,
            row: 2,
            row_count: 2
        })
    ));
    assert_eq!(
        resolver.resolve(PickReadback::new(GpuPickToken::NONE, ticket.generation())),
        Err(PickingError::EmptyToken)
    );
}

#[test]
fn exhausted_page_generations_never_wrap() {
    let mut resolver = resolver(1);
    resolver.slots[0].generation = PickGeneration(u64::MAX);
    let descriptor = PickPageDescriptor::new(
        DatasetId::new(1),
        ChunkId::new(2),
        span(3, 1),
        EntityKind::Atom,
    );
    assert_eq!(
        resolver.request(descriptor),
        Err(PickingError::GenerationExhausted)
    );
}
