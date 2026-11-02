//! Print a family's grammar book: `cargo run -p asterisk --example book -- seed [centuries]`.
use asterisk::{export, world::World};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().unwrap_or_else(|| "tolkien".into());
    let extra: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut w = World::new(&seed);
    for _ in 0..extra {
        w.advance();
    }
    print!("{}", export::markdown(&w, &format!("The {seed} family")));
}
