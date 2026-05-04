use super::*;

#[test]
fn transaction_handle_commits_business_write_and_lease_mutation() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    create_business_table(&wrapper.conn);

    let tx = wrapper.transaction().expect("begin immediate tx");
    tx.conn()
        .execute(
            "INSERT INTO business_events(note) VALUES (?1)",
            params!["tx commit"],
        )
        .expect("insert business event");

    let acquired = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    tx.commit().expect("commit tx");

    assert_eq!(business_event_count(&wrapper.conn), 1);
    assert_eq!(
        core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after tx commit"),
        Some(acquired)
    );
}

#[test]
fn transaction_handle_rolls_back_business_write_and_lease_mutation() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    create_business_table(&wrapper.conn);

    let tx = wrapper.transaction().expect("begin immediate tx");
    tx.conn()
        .execute(
            "INSERT INTO business_events(note) VALUES (?1)",
            params!["tx rollback"],
        )
        .expect("insert business event");
    let acquired = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim");
    assert!(matches!(acquired, ClaimResult::Acquired(_)));

    tx.rollback().expect("rollback tx");

    assert_eq!(business_event_count(&wrapper.conn), 0);
    assert_eq!(
        core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after tx rollback"),
        None
    );
    assert_eq!(
        core::token(&wrapper.conn, "scheduler").expect("token after tx rollback"),
        None
    );
}

#[test]
fn transaction_handle_multiple_mutators_commit_together() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let tx = wrapper.transaction().expect("begin immediate tx");

    let scheduler = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim scheduler");
    let janitor = tx
        .claim("janitor", "worker-b", Duration::from_secs(5))
        .expect("tx claim janitor");

    assert!(matches!(scheduler, ClaimResult::Acquired(_)));
    assert!(matches!(janitor, ClaimResult::Acquired(_)));

    tx.commit().expect("commit tx");

    let now_ms = system_now_for_core();
    assert_eq!(
        core::owner(&wrapper.conn, "scheduler", now_ms).expect("scheduler owner after commit"),
        Some("worker-a".to_owned())
    );
    assert_eq!(
        core::owner(&wrapper.conn, "janitor", now_ms).expect("janitor owner after commit"),
        Some("worker-b".to_owned())
    );
}

#[test]
fn transaction_handle_multiple_mutators_rollback_together() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let tx = wrapper.transaction().expect("begin immediate tx");

    let scheduler = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim scheduler");
    let janitor = tx
        .claim("janitor", "worker-b", Duration::from_secs(5))
        .expect("tx claim janitor");

    assert!(matches!(scheduler, ClaimResult::Acquired(_)));
    assert!(matches!(janitor, ClaimResult::Acquired(_)));

    tx.rollback().expect("rollback tx");

    let now_ms = system_now_for_core();
    assert_eq!(
        core::owner(&wrapper.conn, "scheduler", now_ms).expect("scheduler owner after rollback"),
        None
    );
    assert_eq!(
        core::owner(&wrapper.conn, "janitor", now_ms).expect("janitor owner after rollback"),
        None
    );
    assert_eq!(
        core::token(&wrapper.conn, "scheduler").expect("scheduler token after rollback"),
        None
    );
    assert_eq!(
        core::token(&wrapper.conn, "janitor").expect("janitor token after rollback"),
        None
    );
}

#[test]
fn dropping_transaction_handle_without_commit_rolls_back() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    create_business_table(&wrapper.conn);

    {
        let tx = wrapper.transaction().expect("begin immediate tx");
        tx.conn()
            .execute(
                "INSERT INTO business_events(note) VALUES (?1)",
                params!["tx drop rollback"],
            )
            .expect("insert business event");
        let acquired = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim");
        assert!(matches!(acquired, ClaimResult::Acquired(_)));
    }

    assert_eq!(business_event_count(&wrapper.conn), 0);
    assert_eq!(
        core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after dropped tx"),
        None
    );
}

#[test]
fn transaction_handle_preserves_lease_semantics() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let tx = wrapper.transaction().expect("begin immediate tx");

    let first_claim = tx
        .claim("scheduler", "worker-a", Duration::from_millis(20))
        .expect("first tx claim");
    let first_claim = match first_claim {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    let busy_claim = tx
        .claim("scheduler", "worker-b", Duration::from_millis(20))
        .expect("busy tx claim");
    match busy_claim {
        ClaimResult::Busy(current) => {
            assert_eq!(current.owner, "worker-a");
            assert_eq!(current.token, first_claim.token);
        }
        ClaimResult::Acquired(lease) => panic!("unexpected acquired lease: {lease:?}"),
    }

    let takeover = core::claim_in_tx(
        tx.conn(),
        "scheduler",
        "worker-b",
        first_claim.lease_expires_at_ms + 1,
        20,
    )
    .expect("deterministic takeover tx claim");
    let takeover = match takeover {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };
    assert_eq!(takeover.token, first_claim.token + 1);

    let released = tx.release("scheduler", "worker-b").expect("tx release");
    assert_eq!(
        released,
        ReleaseResult::Released {
            name: "scheduler".to_owned(),
            token: takeover.token,
        }
    );

    let reclaimed = tx
        .claim("scheduler", "worker-c", Duration::from_millis(200))
        .expect("reclaim tx claim");
    let reclaimed = match reclaimed {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };
    assert_eq!(reclaimed.token, takeover.token + 1);

    tx.commit().expect("commit tx");

    let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
        .expect("inspect after tx semantic stress");
    assert_eq!(inspected, Some(reclaimed.clone()));
    assert_eq!(
        core::token(&wrapper.conn, "scheduler").expect("token after tx semantic stress"),
        Some(reclaimed.token)
    );
}

#[test]
fn transaction_handle_renew_extends_existing_lease() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let tx = wrapper.transaction().expect("begin immediate tx");

    let acquired = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    let renewed = tx
        .renew("scheduler", "worker-a", Duration::from_secs(60))
        .expect("tx renew owner-match");
    let renewed = match renewed {
        RenewResult::Renewed(lease) => lease,
        RenewResult::Rejected { current } => {
            panic!("unexpected renew rejection: {current:?}")
        }
    };
    assert_eq!(renewed.token, acquired.token);
    assert_eq!(renewed.owner, acquired.owner);
    assert!(renewed.lease_expires_at_ms >= acquired.lease_expires_at_ms);

    let wrong_owner = tx
        .renew("scheduler", "worker-b", Duration::from_secs(60))
        .expect("tx renew wrong-owner");
    match wrong_owner {
        RenewResult::Rejected {
            current: Some(current),
        } => {
            assert_eq!(current.owner, "worker-a");
            assert_eq!(current.token, renewed.token);
        }
        other => panic!("unexpected wrong-owner renew result: {other:?}"),
    }

    let missing = tx
        .renew("never-claimed", "worker-a", Duration::from_secs(60))
        .expect("tx renew missing-resource");
    assert_eq!(missing, RenewResult::Rejected { current: None });

    tx.commit().expect("commit tx");

    let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
        .expect("inspect after tx renew commit");
    assert_eq!(inspected, Some(renewed));
}

#[test]
fn transaction_handle_inspect_returns_live_lease() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let tx = wrapper.transaction().expect("begin immediate tx");

    assert_eq!(
        tx.inspect("scheduler").expect("tx inspect before claim"),
        None
    );

    let acquired = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    let inspected = tx
        .inspect("scheduler")
        .expect("tx inspect after claim")
        .expect("expected live lease inside tx");
    assert_eq!(inspected, acquired);

    assert_eq!(
        tx.inspect("never-claimed")
            .expect("tx inspect missing resource"),
        None
    );

    tx.commit().expect("commit tx");
}

#[test]
fn transaction_handle_commit_is_visible_to_fresh_connection() {
    let (tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    configure_test_connection(&wrapper.conn);

    let tx = wrapper.transaction().expect("begin immediate tx");
    let acquired = tx
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("tx claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };
    tx.commit().expect("commit tx");

    let fresh = Connection::open(tempdir.path().join("litelease.sqlite3")).expect("open fresh conn");
    configure_test_connection(&fresh);
    let inspected = core::inspect(&fresh, "scheduler", system_now_for_core())
        .expect("inspect from fresh conn after commit");
    assert_eq!(inspected, Some(acquired));
}

#[test]
fn savepoint_handle_rollback_discards_lease_mutation() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let mut tx = wrapper.transaction().expect("begin immediate tx");
    let sp = tx.savepoint().expect("open savepoint");

    let acquired = sp
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("savepoint claim");
    assert!(matches!(acquired, ClaimResult::Acquired(_)));
    assert!(sp
        .inspect("scheduler")
        .expect("inspect inside savepoint after claim")
        .is_some());

    sp.rollback().expect("rollback savepoint");

    tx.commit().expect("commit outer tx");

    assert_eq!(
        core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after rolled-back savepoint"),
        None
    );
}

#[test]
fn savepoint_handle_commit_persists_after_outer_commit() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    create_business_table(&wrapper.conn);

    let mut tx = wrapper.transaction().expect("begin immediate tx");
    let sp = tx.savepoint().expect("open savepoint");

    sp.conn()
        .execute(
            "INSERT INTO business_events(note) VALUES (?1)",
            params!["savepoint commit"],
        )
        .expect("insert business event");
    let acquired = sp
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("savepoint claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    sp.commit().expect("commit savepoint");
    tx.commit().expect("commit outer tx");

    assert_eq!(business_event_count(&wrapper.conn), 1);
    let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
        .expect("inspect after committed savepoint");
    assert_eq!(inspected, Some(acquired));
}

#[test]
fn savepoint_handle_renew_extends_existing_lease() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let mut tx = wrapper.transaction().expect("begin immediate tx");
    let sp = tx.savepoint().expect("open savepoint");

    let acquired = sp
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("savepoint claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    let renewed = sp
        .renew("scheduler", "worker-a", Duration::from_secs(60))
        .expect("savepoint renew owner-match");
    let renewed = match renewed {
        RenewResult::Renewed(lease) => lease,
        RenewResult::Rejected { current } => {
            panic!("unexpected renew rejection: {current:?}")
        }
    };
    assert_eq!(renewed.token, acquired.token);
    assert_eq!(renewed.owner, acquired.owner);
    assert!(renewed.lease_expires_at_ms >= acquired.lease_expires_at_ms);

    let wrong_owner = sp
        .renew("scheduler", "worker-b", Duration::from_secs(60))
        .expect("savepoint renew wrong-owner");
    match wrong_owner {
        RenewResult::Rejected {
            current: Some(current),
        } => {
            assert_eq!(current.owner, "worker-a");
            assert_eq!(current.token, renewed.token);
        }
        other => panic!("unexpected wrong-owner renew result: {other:?}"),
    }

    let missing = sp
        .renew("never-claimed", "worker-a", Duration::from_secs(60))
        .expect("savepoint renew missing-resource");
    assert_eq!(missing, RenewResult::Rejected { current: None });

    sp.commit().expect("commit savepoint");
    tx.commit().expect("commit outer tx");

    let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
        .expect("inspect after savepoint renew commit");
    assert_eq!(inspected, Some(renewed));
}

#[test]
fn savepoint_handle_release_clears_live_owner() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");

    let mut tx = wrapper.transaction().expect("begin immediate tx");
    let sp = tx.savepoint().expect("open savepoint");

    let acquired = sp
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("savepoint claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    let released = sp
        .release("scheduler", "worker-a")
        .expect("savepoint release");
    assert_eq!(
        released,
        ReleaseResult::Released {
            name: "scheduler".to_owned(),
            token: acquired.token,
        }
    );
    assert_eq!(
        sp.inspect("scheduler")
            .expect("inspect inside savepoint after release"),
        None
    );

    sp.commit().expect("commit savepoint");
    tx.commit().expect("commit outer tx");

    assert_eq!(
        core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after savepoint release commit"),
        None
    );
    assert_eq!(
        core::token(&wrapper.conn, "scheduler").expect("token after savepoint release commit"),
        Some(acquired.token)
    );
}

#[test]
fn savepoint_commit_then_outer_rollback_discards_changes() {
    let (_tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    create_business_table(&wrapper.conn);

    let mut tx = wrapper.transaction().expect("begin immediate tx");
    let sp = tx.savepoint().expect("open savepoint");

    sp.conn()
        .execute(
            "INSERT INTO business_events(note) VALUES (?1)",
            params!["savepoint commit outer rollback"],
        )
        .expect("insert business event");
    let acquired = sp
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("savepoint claim");
    assert!(matches!(acquired, ClaimResult::Acquired(_)));

    sp.commit().expect("commit savepoint");
    tx.rollback().expect("rollback outer tx");

    assert_eq!(business_event_count(&wrapper.conn), 0);
    assert_eq!(
        core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after outer rollback"),
        None
    );
    assert_eq!(
        core::token(&wrapper.conn, "scheduler").expect("token after outer rollback"),
        None
    );
}

#[test]
fn savepoint_commit_is_visible_to_fresh_connection_after_outer_commit() {
    let (tempdir, mut wrapper) = open_wrapper_db();
    wrapper.bootstrap().expect("bootstrap");
    configure_test_connection(&wrapper.conn);

    let mut tx = wrapper.transaction().expect("begin immediate tx");
    let sp = tx.savepoint().expect("open savepoint");
    let acquired = sp
        .claim("scheduler", "worker-a", Duration::from_secs(5))
        .expect("savepoint claim");
    let acquired = match acquired {
        ClaimResult::Acquired(lease) => lease,
        ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
    };

    sp.commit().expect("commit savepoint");
    tx.commit().expect("commit outer tx");

    let fresh = Connection::open(tempdir.path().join("litelease.sqlite3")).expect("open fresh conn");
    configure_test_connection(&fresh);
    let inspected = core::inspect(&fresh, "scheduler", system_now_for_core())
        .expect("inspect from fresh conn after savepoint commit");
    assert_eq!(inspected, Some(acquired));
}
