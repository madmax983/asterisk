//! Irregular forms after a long run: `cargo run -p asterisk --example paradigms -- seed`.
use asterisk::world::World;

fn main() {
    let seed = std::env::args().nth(1).unwrap_or_else(|| "asterisk".into());
    let mut w = World::new(&seed);
    for _ in 0..90 {
        w.advance();
    }
    for &id in w.spoken().iter().take(3) {
        let l = &w.langs[id.index()];
        println!("{} — {} irregular", l.name(), w.irregulars(id).len());
        for (m, b, a) in w.vocab.paradigm_entries().take(14) {
            let reg = w.regular(id, m).map(|r| r.to_string()).unwrap_or_default();
            let _ = a;
            let cur = l.lexicon[m.index()].to_string();
            let mark = if reg == cur { "" } else { "  ← irregular" };
            println!(
                "  {:<10} {:<10} +{:<4} = {:<12} actual {}{mark}",
                w.gloss(m),
                l.lexicon[b.index()].to_string(),
                l.lexicon[a.index()].to_string(),
                reg,
                cur
            );
        }
    }
    let lv = w
        .langs
        .iter()
        .flat_map(|l| &l.history)
        .filter(|e| matches!(e.what, asterisk::world::Happening::Levelling { .. }))
        .count();
    println!("levellings: {lv}");
}
