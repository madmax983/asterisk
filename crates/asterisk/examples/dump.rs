//! Dump worlds as text, for checking ports: `cargo run -p asterisk --example dump -- seed...`.
use asterisk::lexicon::Word;
use asterisk::world::{Fate, World};

fn main() {
    for seed in std::env::args().skip(1) {
        let mut w = World::new(&seed);
        let _ = w.apply_hand("V > [+nasal] / _N", None, 200);
        for _ in 0..20 {
            w.advance();
        }
        println!("seed {seed} century {} langs {}", w.century, w.langs.len());
        for l in &w.langs {
            let fate = match l.fate {
                Fate::Spoken => "spoken".to_owned(),
                Fate::Split(c) => format!("split{c}"),
                Fate::Silent(c) => format!("silent{c}"),
            };
            let lex: Vec<String> = l.lexicon.iter().map(Word::to_string).collect();
            println!(
                "{} {:?} {} {} {} | {}",
                l.id,
                l.parent.map(|p| p.0),
                fate,
                l.hue,
                l.name(),
                lex.join(" ")
            );
        }
        for f in &w.feed {
            println!(
                "feed {} {} {}",
                f.century,
                f.text,
                f.example.clone().unwrap_or_default()
            );
        }
        if let Some(m) = w.vocab.named("moon") {
            for s in w.etymology(w.spoken()[0], m) {
                println!(
                    "ety {} {} {} {} {:?}",
                    s.century, s.before, s.after, s.cause, s.rule
                );
            }
        }
        println!("liveliest {:?}", w.liveliest().map(|m| w.gloss(m)));
    }
}
