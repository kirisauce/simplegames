#![feature(panic_info_message)]

mod grid;
mod render;

pub use grid::*;
pub use render::*;

use std::boxed::Box;
use std::panic::*;

fn main() {
    let no_game_no_life = Game::new();

    set_hook(Box::new(|panic_info| {
        Game::cleanup();
        if let Some(location) = panic_info.location() {
            println!("The program is panicking.");
            println!("at {}:{}", location.file(), location.line());
            println!("{}", panic_info);
        } else {
            println!("The program is panicking but there is no info about where it occurred");
        }
    }));

    Game::init();

    no_game_no_life.render_game();

    Game::cleanup();
}
