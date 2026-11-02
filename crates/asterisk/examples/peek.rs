//! Print a world: `cargo run -p asterisk --example peek -- moon`.
use asterisk::world::World;

fn main() {
    let seed = std::env::args().nth(1).unwrap_or_else(|| "moon".into());
    let w = World::new(&seed);
    println!(
        "century {}  langs {}  spoken {}",
        w.century,
        w.langs.len(),
        w.spoken().len()
    );
    for g in ["moon", "sun", "water", "mother", "fire", "tongue"] {
        let Some(m) = w.vocab.named(g) else { continue };
        print!("{g:>8}: *{:<10}", w.proto[m.index()].to_string());
        for id in w.leaf_order() {
            print!(
                " {:<10}",
                w.langs[id.index()].lexicon[m.index()].to_string()
            );
        }
        println!();
    }
    for id in w.leaf_order() {
        println!(
            "  {} ({:?})",
            w.langs[id.index()].name(),
            w.langs[id.index()].fate
        );
    }
    let id = w.spoken()[0];
    if let Some(m) = w.vocab.named("moon") {
        for s in w.etymology(id, m) {
            println!(
                "  c{} {} {} > {}  [{}] {:?}",
                s.century, s.lang_name, s.before, s.after, s.cause, s.rule
            );
        }
    }
    for l in w.feed.iter().rev().take(8) {
        println!("  · c{} {} {:?}", l.century, l.text, l.example);
    }
}
