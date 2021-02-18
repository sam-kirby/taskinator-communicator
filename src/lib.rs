#![deny(
    clippy::all,
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    warnings
)]

#[allow(clippy::all, clippy::pedantic)]
pub mod bindings {
    ::windows::include_bindings!();
}

pub mod error;
pub mod game;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

// const RETRY_DELAY: u64 = 5;

// fn main() -> Result<()> {
//     SimpleLogger::new().with_level(log::LevelFilter::Info).init()?;

//     let mut system = System::new_with_specifics(RefreshKind::new().with_processes());

//     let among_us_pid = loop {
//         system.refresh_processes();

//         if let Some(among_us_proc) = system.get_process_by_name("Among Us.exe").first() {
//             break among_us_proc.pid();
//         } else {
//             log::warn!("Failed to find Among Us, is the game running?");
//             log::warn!("Retrying in {} seconds", RETRY_DELAY);
//             sleep(Duration::from_secs(RETRY_DELAY));
//         }
//     };

//     log::info!("Found Among Us with PID = {}", among_us_pid);
//     log::info!("Attempting to connect to Among Us process...");

//     let game = unsafe { Game::from_pid(among_us_pid) }?;

//     log::info!("Got a connection to the game!");

//     let mut last_read_succeeded = true;
//     loop {
//         match game.state() {
//             Ok(state) => {
//                 last_read_succeeded = true;
//                 log::info!("{:?}", state);
//             }
//             Err(why) => {
//                 if last_read_succeeded {
//                     last_read_succeeded = false;
//                     log::warn!("Failed to read the game state; \
//                     this is normal when game is starting or changing levels");
//                     log::warn!("{}", why);
//                 } else {
//                     log::error!("Failed to get the game state twice; aborting!");
//                     break Err(why);
//                 }
//             }
//         }

//         sleep(Duration::from_secs(3));
//     }
// }
