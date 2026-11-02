//! The HTTP surface against a real Autumn app and a real gallery, without
//! the clock: commands run as they arrive (the chronicle has its own test).

use asterisk_app::atlas::Atlas;
use asterisk_app::store::Store;
use autumn_web::config::AutumnConfig;
use autumn_web::test::{TestApp, TestClient};

fn client() -> TestClient {
    let mut config = AutumnConfig {
        profile: Some("test".into()),
        ..AutumnConfig::default()
    };
    // The browser sends the token in a header; the CSRF layer is Autumn's own
    // and is exercised end to end by verify.sh, not here.
    config.security.csrf.enabled = false;
    TestApp::new()
        .config(config)
        .routes(asterisk_app::all_routes!())
        .state_initializer(|state| {
            let atlas = Atlas::open(Store::in_memory().expect("a log")).expect("a gallery");
            atlas
                .create(&asterisk_app::first_family(6), 1)
                .expect("the first family");
            asterisk_app::install_without_clock(state, atlas);
        })
        .build()
}

fn json(r: &autumn_web::test::TestResponse) -> serde_json::Value {
    serde_json::from_str(&r.text()).expect("a JSON body")
}

const F: &str = "/f/the-asterisk-family";

#[tokio::test]
async fn every_page_serves() {
    let c = client();
    c.get("/")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("The Asterisk family");
    c.get("/new")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("Grow a family");
    c.get(F)
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("svg class=\"tree\"");
    c.get(&format!("{F}?m=salt"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("‘salt’");
    c.get(&format!("{F}?m=you%20all"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("‘you all’");
    c.get(&format!("{F}/_tree?m=foot.PL"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("‘foot.PL’");
    c.get(&format!("{F}/_feed"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("class=\"feed\"");
    c.get(&format!("{F}/_targets"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("every spoken language");
    c.get(&format!("{F}/lang/0"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("Grammar");
    c.get(&format!("{F}/lang/1/word/moon"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("Its cognates");
    c.get("/about")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("Writing a word list");
    c.get("/asterisk.js")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("window.asterisk");
}

#[tokio::test]
async fn missing_things_are_404() {
    let c = client();
    c.get("/f/nobody").send().await.assert_status(404);
    c.get(&format!("{F}/lang/9999"))
        .send()
        .await
        .assert_status(404);
    c.get(&format!("{F}/lang/0/word/spaceship"))
        .send()
        .await
        .assert_status(404);
    c.get(&format!("{F}?m=spaceship"))
        .send()
        .await
        .assert_status(404);
}

#[tokio::test]
async fn the_family_starts_after_its_prelude_and_replays() {
    let c = client();
    let s = json(&c.get(&format!("{F}/_world.json")).send().await);
    assert_eq!(s["seed"], "asterisk");
    assert_eq!(s["century"], 32);
    assert_eq!(s["commands"], 0);
    assert_eq!(s["replays"], true);
    assert!(s["meanings"].as_u64().unwrap_or(0) > 250);
}

#[tokio::test]
async fn centuries_and_rules_join_the_log_and_replay() {
    let c = client();
    for i in 1..=4 {
        let s = json(&c.post(&format!("{F}/century")).send().await);
        assert_eq!(s["century"], 32 + i);
        assert_eq!(s["commands"], i);
        assert_eq!(s["replays"], true);
    }
    let before = c.get(&format!("{F}/_tree?m=moon")).send().await.text();
    c.post(&format!("{F}/preview"))
        .form("rule=V+%3E+%E2%88%85+%2F+C_%23&target=all")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("would change");
    assert_eq!(
        c.get(&format!("{F}/_tree?m=moon")).send().await.text(),
        before,
        "a preview changes nothing"
    );
    c.post(&format!("{F}/apply"))
        .form("rule=V+%3E+%5B%2Bnasal%5D+%2F+_N&target=all&hue=200")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("sent to the chronicle");
    let s = json(&c.get(&format!("{F}/_world.json")).send().await);
    assert_eq!(s["commands"], 5);
    assert_eq!(s["replays"], true);
    let feed = c.get(&format!("{F}/_feed")).send().await.text();
    assert!(feed.contains("by hand"), "{feed}");
}

#[tokio::test]
async fn a_bad_rule_is_explained_not_logged() {
    let c = client();
    c.post(&format!("{F}/preview"))
        .form("rule=p+f&target=all")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("expected");
    c.post(&format!("{F}/apply"))
        .form("rule=%5B%2Bwet%5D+%3E+p&target=all")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("no feature called +wet");
    c.post(&format!("{F}/apply"))
        .form("rule=p+%3E+f&target=banana")
        .send()
        .await
        .assert_status(400);
    c.post(&format!("{F}/preview"))
        .form("rule=p+%3E+f&target=0")
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("no longer spoken");
    assert_eq!(
        json(&c.get(&format!("{F}/_world.json")).send().await)["commands"],
        0
    );
}

#[tokio::test]
async fn families_are_grown_from_words_and_lists() {
    let c = client();
    let r = c
        .post("/families")
        .form("title=The+Tolkien+family&seed=beren&span=100&century_secs=2")
        .send()
        .await;
    r.assert_status(200);
    let created = json(&r);
    assert_eq!(created["slug"], "the-tolkien-family");
    let s = json(&c.get("/f/the-tolkien-family/_world.json").send().await);
    assert_eq!(s["seed"], "beren");
    assert_eq!(s["span"], 100);

    let list = "moon+%3D+mena%0Astar%0APL+%3D+s%0Astar.PL";
    let r = c
        .post("/families")
        .form(&format!("title=Mine&seed=x&vocab={list}"))
        .send()
        .await;
    r.assert_status(200);
    let s = json(&c.get("/f/mine/_world.json").send().await);
    assert_eq!(s["meanings"], 6);
    c.get("/f/mine?m=star.PL").send().await.assert_status(200);

    c.post("/families")
        .form("title=Bad&seed=x&vocab=star.PL")
        .send()
        .await
        .assert_status(400);
    c.post("/families")
        .form("title=&seed=x")
        .send()
        .await
        .assert_status(400);
    let gallery = c.get("/").send().await.text();
    assert!(gallery.contains("The Tolkien family") && gallery.contains("Mine"));
}

#[tokio::test]
async fn exports_download_as_markdown_and_json() {
    let c = client();
    let r = c.get(&format!("{F}/export.md")).send().await;
    r.assert_status(200)
        .assert_body_contains("```mermaid")
        .assert_body_contains("## Cognates");
    let r = c.get(&format!("{F}/export.json")).send().await;
    r.assert_status(200);
    let v = json(&r);
    assert_eq!(v["format"], "asterisk-family/1");
    assert_eq!(v["seed"], "asterisk");
    c.get(&format!("{F}/lang/1/export.md"))
        .send()
        .await
        .assert_status(200)
        .assert_body_contains("### Sound changes, in order");
}

#[tokio::test]
async fn time_starts_and_stops() {
    let c = client();
    assert_eq!(
        json(&c.post(&format!("{F}/drift/off")).send().await)["drifting"],
        false
    );
    assert_eq!(
        json(&c.post(&format!("{F}/drift/on")).send().await)["drifting"],
        true
    );
    c.post(&format!("{F}/drift/sideways"))
        .send()
        .await
        .assert_status(400);
}
