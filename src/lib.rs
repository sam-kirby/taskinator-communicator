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
