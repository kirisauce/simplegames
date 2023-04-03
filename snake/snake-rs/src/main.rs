pub mod game;

fn main() {
    #[cfg(debug_assertions)]
    {
    use std::panic::set_hook;
    use std::fs::OpenOptions;
    use std::io::Write;

    set_hook(Box::new(|info| {
        let f_ret = OpenOptions::new()
            .write(true)
            .create(true)
            .open("panic.log");
        if let Ok(mut f) = f_ret {
            let _ = f.write_all(format!("Error: {:?}", info).as_bytes());
        }
    }));
    }

    let err;
    let count;
    {
        let mut no_game_no_life = game::SnakeGame::new(20, 20, 2);

        err = no_game_no_life.game_loop();

        count = no_game_no_life.count_bodies();
    }

    if let Err(msg) = err {
        if msg == "Game over!" {
            println!("Game over, Score: {}", count);
        } else {
            println!("Game over, Score: {}\nBecause {}", count, msg);
        }
    } else {
        println!("Game over, Score: {}", count);
    }
}
