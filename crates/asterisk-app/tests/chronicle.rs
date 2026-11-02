//! The clock, for real: Harvest on a `SQLite` file drives a family's
//! centuries, carries a typed rule in as a signal, dies, and resumes from
//! its file — and the command log rebuilds exactly the world it left.

use std::sync::Arc;
use std::time::Duration;

use asterisk::world::Command;
use asterisk_app::atlas::{Atlas, NewFamily};
use asterisk_app::chronicle::{ChronicleInput, Driver};
use asterisk_app::store::Store;

fn family() -> NewFamily {
    NewFamily {
        title: "Clockwork".into(),
        seed: "clockwork".into(),
        vocab: None,
        span: Some(40),
        century_secs: 1,
    }
}

async fn until(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while !ok() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chronicle_ticks_takes_rules_survives_a_restart_and_completes() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let log = dir.path().join("log.db");
    let harvest = dir.path().join("harvest.db");

    // ── First life ──
    let atlas = Arc::new(Atlas::open(Store::open(&log).expect("log")).expect("gallery"));
    let f = atlas.create(&family(), 1).expect("a family");
    let slug = f.row.slug.clone();
    let driver = Driver::spawn(Some(&harvest), Arc::clone(&atlas), None).expect("a driver");
    driver
        .start(ChronicleInput {
            slug: slug.clone(),
            century_secs: 1,
            next: 0,
        })
        .await
        .expect("started");
    until("two centuries", 10, || f.applied() >= 2).await;

    // A typed rule arrives as a signal and joins the log in order.
    let before = f.applied();
    let hand = Command::Hand {
        rule: "V > [+nasal] / _N".into(),
        target: None,
        hue: 120,
    };
    assert!(
        driver
            .send(&slug, hand.clone(), "form-1".into())
            .await
            .expect("sent")
    );
    assert!(
        !driver
            .send(&slug, hand.clone(), "form-1".into())
            .await
            .expect("resent"),
        "a resubmitted form is not delivered twice"
    );
    until("the rule", 10, || {
        atlas.store().commands(&slug).is_ok_and(|l| {
            l.iter()
                .skip(usize::try_from(before).unwrap_or(0))
                .any(|c| c.cmd == hand)
        })
    })
    .await;
    let hands = atlas
        .store()
        .commands(&slug)
        .expect("log")
        .iter()
        .filter(|c| c.cmd == hand)
        .count();
    assert_eq!(hands, 1);

    // ── Death ── drop the driver mid-chronicle; the task and its file lock go.
    drop(driver);
    let at_death = f.applied();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    drop(f);
    drop(atlas);

    // ── Second life ── a fresh gallery from the log, a fresh runtime on the
    // same Harvest file. Starting with the same workflow id attaches to the
    // run that was in flight.
    let atlas = Arc::new(Atlas::open(Store::open(&log).expect("log")).expect("gallery"));
    let f = atlas.get(&slug).expect("the family survived");
    assert!(f.applied() >= at_death, "nothing logged was lost");
    assert!(f.diverged.is_none());
    let driver = Driver::spawn(Some(&harvest), Arc::clone(&atlas), None).expect("a driver again");
    driver
        .start(ChronicleInput {
            slug: slug.clone(),
            century_secs: 1,
            next: f.applied(),
        })
        .await
        .expect("attached");

    // Prelude is 32 and the span 40: eight logged centuries complete it.
    until("the end", 30, || f.world().complete()).await;
    assert_eq!(f.world().century, 40);
    let log_now = atlas.store().commands(&slug).expect("log");
    assert_eq!(
        log_now.iter().filter(|c| c.cmd == Command::Advance).count(),
        8
    );
    assert_eq!(
        log_now.iter().map(|c| c.seq).collect::<Vec<_>>(),
        (0..log_now.len() as u64).collect::<Vec<_>>(),
        "no gaps, no repeats"
    );
    assert!(
        atlas.verify(&slug).expect("verify"),
        "the log alone rebuilds this world"
    );
}
