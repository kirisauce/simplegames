use std::thread;
use std::time::{Instant, Duration};
use std::io::{stdout, Write};
use std::sync::atomic::{Ordering::*, AtomicBool, AtomicI16, AtomicU32};
use std::sync::{Arc, RwLockWriteGuard, RwLock, Mutex, MutexGuard};
use std::ops::DerefMut;
use std::sync::mpsc::channel;
use std::mem::swap;
use crate::grid::*;

use crossterm::{
    event::{
        read as event_read,
        poll as event_poll,
        *
    },
    terminal,
    terminal::size as term_size,
    QueueableCommand,
    execute,
    queue,
    cursor,
    style,
    style::Stylize
};
use unicode_width::UnicodeWidthStr;

macro_rules! arc_borrow_closure {
    (($($vars:ident),*) $code:expr) => {
        {
            $(let $vars = Arc::clone(&$vars);)*
            $code
        }
    }
}

macro_rules! set_panic_hook {
    (($message:expr) $code:block) => {
        std::panic::set_hook(std::boxed::Box::new(|panic_info: &std::panic::PanicInfo| {
            $code
            let l = panic_info.location().unwrap();
            println!("Panicking at {}:{}. Message:", l.file(), l.line());
            println!("{:?}", panic_info.message());
            println!("{:?}", $message);
        }));
    };
    ($code:block) => {
        set_panic_hook!(("") $code);
    };
}

enum GameEvent {
    MoveLeft,
    MoveRight,
    MoveDown,
    RotateClock,
    RotateUnclock,
    Pause,
    DebugBrickPosition,
}

pub struct Game {
    pub config_grid_width: u16,
    pub config_grid_height: u16,
    pub config_window_xcoord: u16,
    pub config_window_ycoord: u16,
    pub config_debug_enabled: bool,
}

impl Game {
    pub fn new()-> Self {
        Self {
            config_grid_width: 10,
            config_grid_height: 20,
            config_window_xcoord: 1,
            config_window_ycoord: 1,
            config_debug_enabled: true,
        }
    }

    pub fn init() {
        terminal::enable_raw_mode().unwrap();
        execute!(
            stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide,
            terminal::SetTitle("Tetris-Rust"),
        ).unwrap();
    }

    pub fn cleanup() {
        execute!(
            stdout(),
            cursor::Show,
            terminal::LeaveAlternateScreen,
        ).unwrap();
        terminal::disable_raw_mode().unwrap();
    }

    pub fn calc_center(screen_len: u16, content_len: u16)-> u16 {
        if screen_len < content_len {
            0
        } else {
            (screen_len - content_len) / 2
        }
    }

    pub fn draw_string_center(row: u16, s: &String) {
        let (width, _) = term_size().unwrap();
        let mut stdout = stdout();
        queue!(
            stdout,
            cursor::SavePosition,
            cursor::MoveTo(Game::calc_center(width, s.width() as u16), row),
            style::Print(s),
            cursor::RestorePosition,
        ).unwrap();
    }

    fn clear_screen() {
        stdout().queue(terminal::Clear(terminal::ClearType::All)).unwrap();
    }

    /// 用到三个线程
    ///  TetrisRender-KeyboardThread(main)
    ///    捕获键盘事件并发送到UpdateThread处理
    ///  TetrisRender-UpdateThread
    ///    用于更新游戏，处理事件
    pub fn render_game(&self) {
        #[allow(unused)]
        let mut ycoord = self.config_window_xcoord;
        #[allow(unused)]
        let mut xcoord = self.config_window_ycoord;
        let gwidth = self.config_grid_width;
        let gheight = self.config_grid_height;
        let width = gwidth * 2 + 2;
        let height = gheight + 2;
        let grid = Arc::new(RwLock::new(Grid2D::<Cell>::new(self.config_grid_width, self.config_grid_height)));
        let brick = Arc::new(RwLock::new(random_brick()));
        let next_brick = Arc::new(Mutex::new(random_brick()));
        let brick_x = Arc::new(AtomicI16::new(Game::calc_center(grid.read().unwrap().width(), brick.read().unwrap().get_active_content_width()) as i16));
        let brick_y = Arc::new(AtomicI16::new(-(brick.read().unwrap().get_active_content_height() as i16)));
        let condition = Arc::new(AtomicBool::new(true));
        let score = Arc::new(AtomicU32::new(0));
        let game_over_flag = Arc::new(AtomicBool::new(false));
        let (continue_notify, continue_trigger) = channel::<()>();
        let dbg_enabled = Arc::new(AtomicBool::new(self.config_debug_enabled));
        let (key_sender, key_receiver) = channel::<GameEvent>();

        let dbg_brick_pos_enabled = Arc::new(AtomicBool::new(false));

        // 绘制标题
        //Game::draw_string_center((size.1 as f64 * 0.2) as u16, self.m_title);

        let draw_border = || {
            // 绘制边框
            let mut stdout = stdout().lock();
            #[allow(unused)]
            let mut s = String::new();
            for y in 0..height {
                for x in 0..width {
                    if x == 0 {
                        if y == 0 {
                            s = "╔".to_string()
                        } else if y == height - 1 {
                            s = "╚".to_string()
                        } else {
                            s = "║".to_string()
                        }
                    } else if x == width - 1 {
                        if y == 0 {
                            s = "╗".to_string()
                        } else if y == height - 1 {
                            s = "╝".to_string()
                        } else {
                            s = "║".to_string()
                        }
                    } else {
                        if y == 0 {
                            s = "═".to_string()
                        } else if y == height - 1 {
                            s = "═".to_string()
                        } else {
                            //"".to_string()
                            continue;
                        }
                    }
                    queue!(stdout,
                        cursor::MoveTo(xcoord + x, ycoord + y),
                        style::Print(s),
                    ).unwrap();
                }
            }
        };

        let _gwidth = gwidth;
        let _gheight = gheight;
        let check_is_inrange = move |x: i16, y: i16|-> bool {
            !(x < 0 || x >= _gwidth as i16 || y >= _gheight as i16)
        };

        let game_over = arc_borrow_closure!(
        (condition, game_over_flag)
        move || {
            game_over_flag.store(true, Release);
            condition.store(false, Release);
        });

        let _gheight = gheight;
        let update_func = arc_borrow_closure!(
        (condition, brick, grid, brick_x, brick_y, dbg_brick_pos_enabled, dbg_enabled, score, next_brick)
        move || {
            set_panic_hook!({});
            // 将砖块存储到网格中，并消除满的一行
            let store_and_new_brick = arc_borrow_closure!(
            (grid, brick_x, brick_y, brick, score, next_brick)
            move ||-> bool {
                let gheight;
                {
                let mut grid = grid.write().unwrap();
                gheight = grid.height();
                let b_x = brick_x.load(Acquire);
                let b_y = brick_y.load(Acquire);
                let mut brick = brick.write().unwrap();
                let _guard = stdout().lock();
                // 将下一个brick与新生成的swap，再将旧的brick写入网格，同时重置坐标
                let mut old_brick = random_brick();
                swap(MutexGuard::deref_mut(&mut next_brick.lock().unwrap()), &mut old_brick);
                swap(RwLockWriteGuard::deref_mut(&mut brick), &mut old_brick);
                let b_grid = old_brick.get_active_grid();
                brick_x.store(Game::calc_center(grid.width(), brick.get_active_content_width()) as i16, Release);
                brick_y.store(-(brick.get_active_content_height() as i16), Release);
                for x in 0..b_grid.width() {
                    for y in 0..b_grid.height() {
                        let src_cell = b_grid.get(x, y).unwrap();
                        if src_cell.has_block() {
                            let r = grid.get_mut((b_x + x as i16) as u16, (b_y + y as i16) as u16);
                            match r {
                                Err(_) => {
                                    game_over();
                                    return true;
                                },
                                Ok(target_cell) => {
                                    target_cell.replace(src_cell.get().clone());
                                },
                            };
                        }
                    }
                }
                }

                // 检测满的行并消除
                let mut y_iter = 1;
                while y_iter <= gheight {
                    let y = gheight - y_iter;
                    let mut is_full = true;
                    {
                    let grid = grid.read().unwrap();
                    for x in 0..gwidth {
                        if !grid.get(x, y).unwrap().has_block() {
                            is_full = false;
                            break;
                        }
                    }
                    }
                    // 消除并下移
                    if is_full {
                        {
                        let mut grid = grid.write().unwrap();
                        for x in 0..grid.width() {
                            grid.get_mut(x, y).unwrap().clear();
                        }
                        for yoffset in 0..y {
                            for x in 0..grid.width() {
                                let upper_cell = grid.get_mut(x, y - yoffset - 1).unwrap() as *mut Cell;
                                if unsafe{&*upper_cell}.has_block() {
                                    swap(unsafe{&mut*upper_cell}, grid.get_mut(x, y - yoffset).unwrap());
                                }
                            }
                        }
                        y_iter -= 1;
                        }
                        score.fetch_add(gwidth as u32, SeqCst);
                        thread::sleep(Duration::from_millis(400));
                    }

                    y_iter += 1;
                }
                false
            });
            // 这个闭包用于将砖块向下移动指定距离
            // 如果遇到障碍就存储
            let move_down = arc_borrow_closure!(
            (brick, brick_x, brick_y, brick, grid)
            move |mut distance: u16|-> bool {
                while distance > 0 {
                    let brick = brick.read().unwrap();
                    let b_x = brick_x.load(Acquire);
                    let b_y = brick_y.load(Acquire);
                    let grid = grid.read().unwrap();
                    for p in brick.get_checking_points(Direction::Down) {
                        let x = b_x + p.0;
                        let y = b_y + p.1;
                        if y < 0 {
                            continue;
                        }
                        if y >= gheight as i16 || grid.get(x as u16, y as u16).unwrap().has_block() {
                            drop(brick);
                            drop(grid);
                            return store_and_new_brick();
                        }
                    }
                    distance -= 1;
                };
                brick_y.fetch_add(1, SeqCst);
                false
            });
            let key_receiver = key_receiver;
            let mut timer = Duration::ZERO;
            while condition.load(Acquire) {
                let begin = Instant::now();

                let event = key_receiver.recv_timeout(Duration::from_millis(50));
                if let Ok(event) = event {
                    match event {
                        GameEvent::DebugBrickPosition => {
                            if dbg_enabled.load(Acquire) {
                                let d = &dbg_brick_pos_enabled;
                                d.store(!d.load(Acquire), Release);
                            }
                        },
                        GameEvent::MoveLeft => {
                            let grid = grid.read().unwrap();
                            let brick = brick.read().unwrap();
                            let b_x = brick_x.load(Acquire);
                            let b_y = brick_y.load(Acquire);
                            let mut blocked = false;
                            for p in brick.get_checking_points(Direction::Left) {
                                let x = b_x + p.0;
                                let y = b_y + p.1;
                                if check_is_inrange(x, y) {
                                    if y < 0 {
                                        continue
                                    }
                                    if grid.get(x as u16, y as u16).unwrap().has_block() {
                                        blocked = true;
                                        break;
                                    }
                                } else {
                                    blocked = true;
                                    break;
                                }
                            }
                            if !blocked {
                                brick_x.fetch_sub(1, SeqCst);
                            }
                        },
                        GameEvent::MoveRight => {
                            let grid = grid.read().unwrap();
                            let brick = brick.read().unwrap();
                            let b_x = brick_x.load(Acquire);
                            let b_y = brick_y.load(Acquire);
                            let mut blocked = false;
                            for p in brick.get_checking_points(Direction::Right) {
                                let x = b_x + p.0;
                                let y = b_y + p.1;
                                if check_is_inrange(x, y) {
                                    if y < 0 {
                                        continue
                                    }
                                    if grid.get(x as u16, y as u16).unwrap().has_block() {
                                        blocked = true;
                                        break;
                                    }
                                } else {
                                    blocked = true;
                                    break;
                                }
                            }
                            if !blocked {
                                brick_x.fetch_add(1, SeqCst);
                            }
                        },
                        GameEvent::MoveDown => {
                            if move_down(1) {
                                continue;
                            }
                        },
                        GameEvent::Pause => {
                            continue_trigger.recv().unwrap();
                        },
                        GameEvent::RotateClock => {
                            let b_x = brick_x.load(Acquire);
                            let b_y = brick_y.load(Acquire);
                            let grid = grid.read().unwrap();
                            let origd;
                            let rd;
                            let r_grid;
                            let mut blocked_list = Vec::<Position>::new();
                            {
                            let brick = brick.read().unwrap();
                            origd = brick.direction();
                            rd = origd.rotate(true);
                            r_grid = brick.get_grid(rd);
                            for x in 0..r_grid.width() {
                                for y in 0..r_grid.height() {
                                    let cell = r_grid.get(x, y).unwrap();
                                    let tmpx = x as i16 + b_x;
                                    let tmpy = y as i16 + b_y;
                                    if cell.has_block() && tmpy >= 0 && (!check_is_inrange(tmpx, tmpy) || grid.get(tmpx as u16, tmpy as u16).unwrap().has_block()) {
                                        blocked_list.push(Position(x as i16, y as i16));
                                    }
                                }
                            }
                            }
                            if blocked_list.is_empty() {
                                brick.write().unwrap().switch(rd);
                            }
                        },
                        _ => {},
                    }
                }

                // 如果累计时间超过400毫秒就执行下落
                timer += begin.elapsed();
                if timer >= Duration::from_millis(400) {
                    if move_down(1) {
                        continue;
                    }
                    timer = Duration::ZERO;
                }
            }
        });

        // 启动更新线程
        let update_thread = thread::Builder::new()
            .name("Tetris-UpdateThread".to_string())
            .spawn(update_func)
            .unwrap();

        // 渲染暂停界面
        // 会阻塞调用的线程直到用户按下继续或退出
        // 若用户按下q键退出则返回true
        let render_pause = arc_borrow_closure!(
        ()
        ||-> bool {
            let mut stdout = stdout();
            loop {
                let cpos = Game::calc_center(term_size().unwrap().1, 2);
                Game::draw_string_center(cpos, &"已暂停".to_string());
                Game::draw_string_center(cpos+1, &"按下Q退出，按下C继续游戏".to_string());
                stdout.flush().unwrap();
                let event_result = event_read();
                if let Ok(event) = event_result {
                    match event {
                        Event::Resize(_, _) => {
                            stdout.queue(terminal::Clear(terminal::ClearType::All)).unwrap();
                        },
                        Event::Key(key) => {
                            match key.code {
                                KeyCode::Char(c) => {
                                    match c {
                                        'c' | 'C' => {
                                            return false;
                                        },
                                        'q' | 'Q' => {
                                            return true;
                                        },
                                        _ => {},
                                    };
                                },
                                _ => {},
                            };
                        },
                        _ => {},
                    }
                }
            };
        });

        // 绘制背景网格
        let draw_grid = || {
            // 锁定网格
            let grid = grid.read().unwrap();
            let mut stdout = stdout();
            // 绘制网格中的砖块
            for y in 0..gheight {
                stdout.queue(cursor::MoveTo(xcoord + 1, ycoord + 1 + y)).unwrap();
                for x in 0..gwidth {
                    let cell = grid.get(x, y).unwrap();
                    if cell.has_block() {
                        stdout.queue(style::Print("  ".on((*cell).m_color))).unwrap();
                    } else {
                        stdout.queue(cursor::MoveRight(2)).unwrap();
                        continue;
                    }
                }
            }
        };

        // 绘制运动的砖块
        let draw_brick = || {
            // 锁定砖块
            let brick = brick.read().unwrap(); // 获取RAII锁
            let grid = brick.get_active_grid(); // 获取活动网格
            let brick_x = brick_x.load(Acquire);
            let brick_y = brick_y.load(Acquire);
            // 绘制砖块
            for x in 0..BRICK_GRID_SIZE {
                for y in 0..BRICK_GRID_SIZE {
                    let cell = grid.get(x, y).unwrap();
                    let tmpx = brick_x + x as i16;
                    let tmpy = brick_y + y as i16;
                    if tmpx < 0 || tmpy < 0 || tmpx >= gwidth as i16 || tmpy >= gheight as i16 {
                        continue;
                    }
                    if cell.has_block() {
                        queue!(
                            stdout(),
                            cursor::MoveTo(
                                xcoord + 1 + (tmpx as u16 * 2),
                                ycoord + 1 + tmpy as u16
                            ),
                            style::Print("  ".on((*cell).m_color))
                        ).unwrap();
                    }
                }
            }
        };

        // 绘制Dashboard的边框
        let draw_dashboard_border = || {
            let w = BRICK_GRID_SIZE * 2 + 4;
            let h = BRICK_GRID_SIZE + 5;
            let top = {
                let mut s = "╔".to_string();
                for _ in 0..(w - 2) {
                    s += "═";
                }
                s += "╗";
                s
            };
            let bottom = {
                let mut s = "╚".to_string();
                for _ in 0..(w - 2) {
                    s += "═";
                }
                s += "╝";
                s
            };
            queue!(
                stdout(),
                cursor::MoveTo(xcoord + width, ycoord),
                style::Print(top),
                cursor::MoveTo(xcoord + width, ycoord + h - 1),
                style::Print(bottom),
            ).unwrap();
            for y in 0..(h - 2) {
                queue!(
                    stdout(),
                    cursor::MoveTo(xcoord + width, ycoord + y + 1),
                    style::Print("║".to_string()),
                    cursor::MoveRight(w - 2),
                    style::Print("║".to_string()),
                ).unwrap();
            }
        };

        // 绘制Dashboard的内容
        let draw_dashboard = arc_borrow_closure!(
        (score, next_brick)
        move || {
            let w = BRICK_GRID_SIZE * 2;
            //let h = BRICK_GRID_SIZE + 3;

            // 绘制分数
            let score_str = format!("分数;{}", score.load(Acquire));
            let strw = score_str.width() as u16;
            if strw > w {
                let line1 = score_str[0..(w as usize)].to_string();
                let line2 = score_str[(w as usize)..].to_string();
                queue!(
                    stdout(),
                    cursor::MoveTo(xcoord + width + 1, ycoord + 1),
                    style::Print(line1),
                    cursor::MoveTo(xcoord + width + 1, ycoord + 2),
                    style::Print(line2),
                ).unwrap();
            } else {
                queue!(
                    stdout(),
                    cursor::MoveTo(xcoord + width + 1, ycoord + 1),
                    style::Print(score_str),
                ).unwrap();
            }

            // 绘制下一个brick
            let mut stdout = stdout();
            let nbrick = next_brick.lock().unwrap();
            let b_grid = nbrick.get_active_grid();
            let xc = Game::calc_center(w, nbrick.get_active_content_width());
            for y in 0..b_grid.height() {
                stdout.queue(cursor::MoveTo(xcoord + width + 1 + xc, ycoord + 4 + y)).unwrap();
                for x in 0..b_grid.width() {
                    let cell = b_grid.get(x, y).unwrap();
                    if cell.has_block() {
                        stdout.queue(style::Print("  ".on((*cell).m_color))).unwrap();
                    } else {
                        stdout.queue(cursor::MoveRight(2)).unwrap();
                    }
                }
            }
        });

        let dbg_draw_brick_pos = |out_y: &mut u16| {
            if dbg_brick_pos_enabled.load(Acquire) {
                queue!(
                    stdout(),
                    cursor::MoveTo(0, *out_y),
                    style::Print(format!("Brick({},{})", brick_x.load(Acquire), brick_y.load(Acquire))),
                ).unwrap();
                *out_y += 1;
            }
        };

        let stop_rendering = || {
            let update_thread = update_thread;
            condition.store(false, Release);
            update_thread.join().unwrap();
        };

        while condition.load(Acquire) {
            Game::clear_screen();
            draw_grid();
            draw_brick();
            draw_border();
            draw_dashboard_border();
            draw_dashboard();
            //draw_dashboard();

            let mut dbg_y = ycoord + height;
            dbg_draw_brick_pos(&mut dbg_y);

            stdout().flush().unwrap();
            if event_poll(Duration::from_millis(100)).unwrap() {
                let event = event_read();
                if let Err(_) = event {
                    continue;
                }
                match event.unwrap() {
                    /*Event::Resize(_, _) => {
                        draw_border();
                        draw_dashboard_border();
                    },*/
                    Event::Key(key) => {
                        match key.code {
                            KeyCode::Char(c) => {
                                if key.modifiers.is_empty() {
                                    match c {
                                        'q' | 'Q' => {
                                            stop_rendering();
                                            return;
                                        },
                                        'p' | 'P' => {
                                            key_sender.send(GameEvent::Pause).unwrap();
                                            if render_pause() {
                                                continue_notify.send(()).unwrap();
                                                stop_rendering();
                                                return;
                                            }
                                            continue_notify.send(()).unwrap();
                                        },
                                        _ => {},
                                    };
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    match c {
                                        'b' => {
                                            key_sender.send(GameEvent::DebugBrickPosition).unwrap();
                                        },
                                        _ => {},
                                    };
                                }
                            },
                            KeyCode::Left => {
                                key_sender.send(GameEvent::MoveLeft).unwrap();
                            },
                            KeyCode::Right => {
                                key_sender.send(GameEvent::MoveRight).unwrap();
                            },
                            KeyCode::Down => {
                                key_sender.send(GameEvent::MoveDown).unwrap();
                            },
                            KeyCode::Up => {
                                key_sender.send(GameEvent::RotateClock).unwrap();
                            },
                            _ => {},
                        };
                    },
                    _ => {},
                };
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
