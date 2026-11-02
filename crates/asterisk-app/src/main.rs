// Asterisk — a language family that drifts while you watch.
//
// Run with:  cargo run --release -p asterisk-app
// Then open: http://localhost:3000   (open it twice: the family is shared)

#[autumn_web::main]
async fn main() {
    Box::pin(
        autumn_web::app()
            .on_startup(asterisk_app::boot)
            .routes(asterisk_app::all_routes!())
            .run(),
    )
    .await;
}
