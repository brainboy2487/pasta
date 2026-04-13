// src/runtime/family/tests.rs
//! Integration tests for the Pasta Family Object System.

#[cfg(test)]
mod tests {
    use crate::runtime::family::{
        FamilyRegistry, NodeRole, ObjGroup, UnsafePermission,
        MissingParent, ReconciliationOutcome,
        FamilyNode, LineageSignature,
        PRIMORDIAL_A_ID, PRIMORDIAL_B_ID,
    };
    use crate::runtime::family::asm::reconcile::{asm_reconcile_returning_parent, tick_shadow_promotion};
    use crate::runtime::family::types::{DeltaEntry, next_family_id};

    fn reg() -> FamilyRegistry {
        FamilyRegistry::new(UnsafePermission::None)
    }

    // ── 1. Primordials ────────────────────────────────────────────────────────

    #[test]
    fn primordials_exist_and_are_immutable() {
        let r = reg();
        assert!(r.primordials.get(PRIMORDIAL_A_ID).is_some());
        assert!(r.primordials.get(PRIMORDIAL_B_ID).is_some());
        assert!(r.primordials.get(99).is_none());
    }

    // ── 2. FamilyNode creation ────────────────────────────────────────────────

    #[test]
    fn create_node_basic() {
        let mut r = reg();
        // Create two parent nodes anchored to primordials
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();

        // Create a child
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();
        assert!(r.get(child).is_some());
        let node = r.get(child).unwrap();
        assert_eq!(node.parent_a_id, pa);
        assert_eq!(node.parent_b_id, pb);
        assert_eq!(node.role, NodeRole::Child);
    }

    #[test]
    fn create_node_type_mismatch_rejected() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Lst,  Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Dict, Some(500), Some(3)).unwrap();
        // LST child with a DICT parent — should be rejected
        let result = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Lst, Some(1000), Some(2));
        assert!(result.is_err(), "expected GroupTypeMismatch error");
    }

    #[test]
    fn csm_can_have_mixed_type_parents() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Lst,  Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Dict, Some(500), Some(3)).unwrap();
        // CSM child with mixed-type parents — should be allowed
        let csm = ObjGroup::Csm { primary: Box::new(ObjGroup::Nrml), extensions: vec![] };
        let result = r.create_node(pa, pb, NodeRole::Child, csm, Some(1000), Some(2));
        assert!(result.is_ok(), "CSM should allow cross-type parents");
    }

    // ── 3. FamilyId auto-increment ────────────────────────────────────────────

    #[test]
    fn family_id_increments() {
        let id1 = next_family_id();
        let id2 = next_family_id();
        assert!(id2 > id1);
    }

    // ── 4. LineageSignature ───────────────────────────────────────────────────

    #[test]
    fn lineage_signature_deterministic() {
        let deltas = vec![
            DeltaEntry { key: 1, value: 10, timestamp: 100 },
            DeltaEntry { key: 2, value: 20, timestamp: 200 },
        ];
        let sig1 = LineageSignature::compute(&deltas, &[], &[]);
        let sig2 = LineageSignature::compute(&deltas, &[], &[]);
        assert_eq!(sig1.hash, sig2.hash);
    }

    #[test]
    fn lineage_signature_changes_with_deltas() {
        let d1 = vec![DeltaEntry { key: 1, value: 10, timestamp: 100 }];
        let d2 = vec![DeltaEntry { key: 1, value: 99, timestamp: 200 }];
        let sig1 = LineageSignature::compute(&d1, &[], &[]);
        let sig2 = LineageSignature::compute(&d2, &[], &[]);
        assert_ne!(sig1.hash, sig2.hash);
    }

    // ── 5. DeltaEntry upsert (latest wins) ───────────────────────────────────

    #[test]
    fn delta_upsert_latest_wins() {
        let mut entries: Vec<DeltaEntry> = vec![];
        FamilyNode::upsert_delta(&mut entries, 42, 10, 100);
        FamilyNode::upsert_delta(&mut entries, 42, 99, 200); // newer — should win
        FamilyNode::upsert_delta(&mut entries, 42,  1,  50); // older — should be ignored
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value, 99);
    }

    // ── 6. Snapshot / restore ─────────────────────────────────────────────────

    #[test]
    fn snapshot_and_restore() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();

        // Add some deltas
        {
            let node = r.get_mut(child).unwrap();
            FamilyNode::upsert_delta(&mut node.mutations, 1, 50, 100);
            node.refresh_lineage();
        }

        let snap = crate::runtime::family::create_snapshot(r.get(child).unwrap());
        assert_eq!(snap.mutation_count, 1);
        assert_eq!(snap.parent_a_id, pa);

        // Corrupt the node then restore
        {
            let node = r.get_mut(child).unwrap();
            FamilyNode::upsert_delta(&mut node.mutations, 2, 77, 200);
            FamilyNode::upsert_delta(&mut node.mutations, 3, 88, 300);
        }
        assert_eq!(r.get(child).unwrap().mutations.len(), 3);

        crate::runtime::family::restore_from_snapshot(r.get_mut(child).unwrap(), &snap);
        assert_eq!(r.get(child).unwrap().mutations.len(), 1);
    }

    // ── 7. Parent check — both visible ───────────────────────────────────────

    #[test]
    fn parent_check_both_visible() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();

        let (a_ok, b_ok) = r.check(child, 1000).unwrap();
        assert!(a_ok);
        assert!(b_ok);
    }

    #[test]
    fn does_parent_exist_true_when_both_visible() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();
        assert!(r.does_parent_exist(child));
    }

    // ── 8. Parent check — triggers adoption when parent gone ─────────────────

    #[test]
    fn parent_check_triggers_adoption_when_parent_missing() {
        let mut r = reg();
        // Create parent A and B
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        // Create a second parent to serve as replacement candidate
        let _spare = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();

        // Remove parent A from registry
        r.nodes.remove(&pa);

        // Run 2 check cycles (threshold = 2) to trigger adoption
        let _ = r.check(child, 1000);
        let _ = r.check(child, 2000);

        // After adoption, child's parent_a_id should have changed
        let new_pa = r.get(child).unwrap().parent_a_id;
        assert_ne!(new_pa, pa, "parent A should have been replaced after adoption");
    }

    // ── 9. DOES_PARENT_EXIST false when parent missing ────────────────────────

    #[test]
    fn does_parent_exist_false_when_parent_removed() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(3)).unwrap();

        assert!(r.does_parent_exist(child));
        r.nodes.remove(&pa);
        assert!(!r.does_parent_exist(child));
    }

    // ── 10. Backward propagation (push_updates) ───────────────────────────────

    #[test]
    fn backward_propagation_pushes_deltas_to_parent() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();

        // Add a mutation to child
        {
            let node = r.get_mut(child).unwrap();
            FamilyNode::upsert_delta(&mut node.mutations, 7, 42, 500);
        }

        // Parent check will push updates
        r.check(child, 1000).unwrap();

        // Parent A should now have the delta
        let parent = r.get(pa).unwrap();
        let found = parent.mutations.iter().any(|d| d.key == 7 && d.value == 42);
        assert!(found, "parent A should have received child mutation delta");
    }

    // ── 11. Reconciliation ────────────────────────────────────────────────────

    #[test]
    fn reconciliation_adoptive_wins_on_newer_timestamp() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(3)).unwrap();

        // Create an adoptive parent with a newer timestamp
        let adoptive = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        r.get_mut(adoptive).unwrap().last_parent_check = 9000;

        // Returning parent has older timestamp
        let returning = pa;
        r.get_mut(returning).unwrap().last_parent_check = 100;

        let outcome = asm_reconcile_returning_parent(
            child, adoptive, returning, MissingParent::A,
            &mut r.nodes, &r.bus, 10000,
        );
        assert!(matches!(outcome, ReconciliationOutcome::ShadowParentCreated(_)),
            "adoptive should win with newer timestamp; returning demoted to shadow");
    }

    #[test]
    fn reconciliation_returning_wins_on_newer_timestamp() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(3)).unwrap();

        let adoptive = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        r.get_mut(adoptive).unwrap().last_parent_check = 100;

        let returning = pa;
        r.get_mut(returning).unwrap().last_parent_check = 9000; // returning is newer

        let outcome = asm_reconcile_returning_parent(
            child, adoptive, returning, MissingParent::A,
            &mut r.nodes, &r.bus, 10000,
        );
        assert!(matches!(outcome, ReconciliationOutcome::ReturningParentWins),
            "returning parent should win with newer timestamp");
    }

    // ── 12. Shadow parent auto-promotion ─────────────────────────────────────

    #[test]
    fn shadow_parent_promotes_after_3x_threshold() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(2)).unwrap();

        // Manually set a shadow parent for slot A (threshold=2, so promote after 6 cycles)
        let shadow_id = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(2)).unwrap();
        {
            let c = r.get_mut(child).unwrap();
            c.shadow_a_id     = Some(shadow_id);
            c.shadow_a_cycles = 0;
        }

        // Tick 5 times — should NOT promote yet (threshold 2 × 3 = 6)
        for i in 0..5 {
            tick_shadow_promotion(child, &mut r.nodes, &r.bus, (i+1) * 1000);
        }
        assert_eq!(r.get(child).unwrap().shadow_a_id, Some(shadow_id), "should still be shadow after 5 ticks");

        // Tick once more (6th tick) — should promote
        tick_shadow_promotion(child, &mut r.nodes, &r.bus, 6000);
        assert!(r.get(child).unwrap().shadow_a_id.is_none(), "shadow should be promoted after 6 ticks");
        assert_eq!(r.get(child).unwrap().parent_a_id, shadow_id, "shadow should be active parent now");
    }

    // ── 13. GC eligibility ────────────────────────────────────────────────────

    #[test]
    fn gc_collectible_when_parent_missing() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(3)).unwrap();

        r.nodes.remove(&pa);
        r.nodes.remove(&pb);

        assert!(crate::runtime::family::gc_is_collectible(
            r.get(child).unwrap(), &r.nodes, false
        ));
    }

    #[test]
    fn gc_pass_removes_collectible_nodes() {
        let mut r = reg();
        let pa = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let pb = r.create_node(PRIMORDIAL_A_ID, PRIMORDIAL_B_ID, NodeRole::Parent, ObjGroup::Nrml, Some(500), Some(3)).unwrap();
        let child = r.create_node(pa, pb, NodeRole::Child, ObjGroup::Nrml, Some(1000), Some(3)).unwrap();

        r.retire(child);
        assert!(r.nodes.contains_key(&child));
        r.gc_pass();
        assert!(!r.nodes.contains_key(&child), "retired node should be collected");
    }

    // ── 14. Permission system ─────────────────────────────────────────────────

    #[test]
    fn global_read_denied_without_permission() {
        let r = reg(); // UnsafePermission::None
        let result = r.bus.subscribe_global_read(
            crate::runtime::family::AdoptionEventType::ParentMissing,
            Box::new(|_| {}),
        );
        assert!(result.is_err(), "should be denied without USE UNSAFE-READ");
    }

    #[test]
    fn global_read_allowed_with_read_permission() {
        let r = FamilyRegistry::new(UnsafePermission::Read);
        let result = r.bus.subscribe_global_read(
            crate::runtime::family::AdoptionEventType::ParentMissing,
            Box::new(|_| {}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn global_write_denied_with_only_read_permission() {
        let r = FamilyRegistry::new(UnsafePermission::Read);
        let result = r.bus.subscribe_global_write(
            crate::runtime::family::AdoptionEventType::ParentMissing,
            Box::new(|_| {}),
        );
        assert!(result.is_err(), "should be denied with only USE UNSAFE-READ");
    }
}
