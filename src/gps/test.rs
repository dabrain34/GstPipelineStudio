use crate::gps::player::Player;
use crate::graphmanager as GM;

#[cfg(test)]
fn test_synced<F, R>(function: F) -> R
where
    F: FnOnce() -> R + Send + std::panic::UnwindSafe + 'static,
    R: Send + 'static,
{
    /// No-op.
    macro_rules! skip_assert_initialized {
        () => {};
    }
    skip_assert_initialized!();

    use futures_channel::oneshot;
    use std::panic;

    let (tx, rx) = oneshot::channel();
    TEST_THREAD_WORKER
        .push(move || {
            tx.send(panic::catch_unwind(function))
                .unwrap_or_else(|_| panic!("Failed to return result from thread pool"));
        })
        .expect("Failed to schedule a test call");
    futures_executor::block_on(rx)
        .expect("Failed to receive result from thread pool")
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

#[cfg(test)]
static TEST_THREAD_WORKER: once_cell::sync::Lazy<gtk::glib::ThreadPool> =
    once_cell::sync::Lazy::new(|| {
        let pool = gtk::glib::ThreadPool::exclusive(1).unwrap();
        pool.push(move || {
            gtk::init().expect("Tests failed to initialize gtk");
        })
        .expect("Failed to schedule a test call");
        pool
    });

#[cfg(test)]
mod player_test {
    use super::*;
    fn check_pipeline(pipeline_desc: &str) {
        let player = Player::new().expect("Not able to create the player");
        let graphview = GM::GraphView::new();
        player.graphview_from_pipeline_description(&graphview, pipeline_desc);
    }

    #[test]
    fn pipeline_creation() {
        test_synced(|| {
            println!("coucou");
            //check_pipeline("videotestsrc ! autovideosink");
        });
    }
}
