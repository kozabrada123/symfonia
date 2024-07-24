mod instance;
mod stats;

use poem::{get, Route};
pub use stats::*;

pub fn setup_routes() -> Route {
    Route::new()
        .nest("/instance", instance::setup_routes())
        .at("/stats", get(stats))
}
