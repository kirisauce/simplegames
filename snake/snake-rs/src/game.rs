use std::ops;
use std::io::{Write, stdout};
use std::time::{Duration, Instant};
use std::sync::Arc;
use crossterm::{
    queue,
    execute,
    QueueableCommand,
    ExecutableCommand,
    terminal::{
        Clear,
        ClearType,
        EnterAlternateScreen,
        LeaveAlternateScreen,
        BeginSynchronizedUpdate,
        EndSynchronizedUpdate,
        SetSize,
        enable_raw_mode,
        disable_raw_mode,
    },
    event::{
        self,
        Event,
        KeyCode,
    },
    style::{
        Print,
    },
    cursor::{
        self,
        MoveTo,
    },
};
use rand::prelude::*;
use rand::thread_rng;



#[derive(Clone, Copy, PartialEq)]
pub struct Position(i16, i16);

impl Position {
    pub fn as_1d(&self, w: i16)-> usize {
        (self.0 + self.1 * w) as usize
    }
}

impl ops::Add for Position {
    type Output = Self;

    fn add(self, other: Self)-> Self::Output {
        Position(self.0 + other.0, self.1 + other.1)
    }
}

impl ops::Sub for Position {
    type Output = Self;

    fn sub(self, other: Self)-> Self::Output {
        Position(self.0 - other.0, self.1 - other.1)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl Direction {
    /// 将方向转换为对应的坐标偏移值
    pub fn to_distance(&self, d: i16)-> Position {
        match *self {
            Direction::Up => Position(0, -d),
            Direction::Right => Position(d, 0),
            Direction::Down => Position(0, d),
            Direction::Left => Position(-d, 0),
        }
    }

    pub fn get_opposite(&self)-> Direction {
        match *self {
            Direction::Up => Direction::Down,
            Direction::Right => Direction::Left,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
        }
    }
}

#[derive(Clone)]
pub struct Body {
    pub m_front: Direction,
    pub m_back: Direction,
}

#[derive(Clone)]
pub enum Cell {
    Empty,
    Body(Body),
    Apple,
    SuperApple,
}



pub struct Map {
    m_vec: Vec<Cell>,
    m_head_pos: Position,
    m_hid_bodies: i16,
    m_width: i16,
    m_height: i16,
}

impl Map {
    pub fn new(w: i16, h: i16, length: i16)-> Self {
        let headpos = Position(w/2, h/2);
        let mut obj = Self {
            m_vec: vec![Cell::Empty; w as usize * h as usize],
            m_head_pos: headpos,
            m_hid_bodies: length - 1,
            m_width: w,
            m_height: h,
        };
        obj.generate_apple(false);
        *obj.get_mut(headpos).unwrap() = Cell::Body(Body {
            m_front: Direction::Up,
            m_back: Direction::Down,
        });
        obj
    }

    pub fn is_pos_valid(&self, pos: &Position)-> bool {
        0 <= pos.0 && 0 <= pos.1 && pos.0 < self.m_width && pos.1 < self.m_height
    }

    pub fn get(&self, pos: Position)-> Option<&Cell> {
        if self.is_pos_valid(&pos) {
            Some(&self.m_vec[pos.as_1d(self.m_width)])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, pos: Position)-> Option<&mut Cell> {
        if self.is_pos_valid(&pos) {
            Some(&mut self.m_vec[pos.as_1d(self.m_width)])
        } else {
            None
        }
    }

    pub fn turn(&mut self, d: Direction)-> bool {
        let head = self.find_head().unwrap();
        if head.m_back == d {
            false // 失败
        } else {
            head.m_front = d;
            true // 成功
        }
    }

    pub fn count_bodies(&self)-> i32 {
        let mut c = 0;
        for i in &self.m_vec {
            if let Cell::Body(_) = i {
                c += 1;
            }
        }
        c
    }

    fn find_head(&mut self)-> Result<&mut Body, String> {
        if let Cell::Body(head) = &mut self.m_vec[self.m_head_pos.as_1d(self.m_width)] {
            Ok(head)
        } else {
            Err("Head not found".to_string())
        }
    }

    pub fn generate_apple(&mut self, force: bool)-> bool {
        let mut rng = thread_rng();
        let mut has_apple = false;
        let mut is_full = true;

        for i in self.m_vec.iter() {
            if let Cell::Apple = i {
                has_apple = true;
                continue;
            }
            if let Cell::Empty = i {
                is_full = false;
                continue;
            }
        }

        if is_full {
            return false;
        }
        if force || !has_apple {
            loop {
                let pos = Position(rng.gen_range(0..self.m_width), rng.gen_range(0..self.m_height));
                let val = self.get_mut(pos).unwrap();
                if let Cell::Empty = val {
                    *val = if rng.gen_range(0..10) > 8 { Cell::SuperApple } else { Cell::Apple };
                    return true
                }
            }
        } else {
            return false;
        }
    }

    pub fn update(&mut self)-> Result<(), String> {
        let front = self.find_head().unwrap().m_front;
        let front_pos = self.m_head_pos + front.to_distance(1);

        let mut has_empty = false;
        for i in self.m_vec.iter() {
            if let Cell::Empty = i {
                has_empty = true;
                break;
            }
        }

        if !has_empty {
            return Err("Game over!".to_string());
        }

        // 判断撞到了什么东西
        match self.get_mut(front_pos) {
            // 撞到了苹果
            Some(Cell::Apple) => {
                *self.get_mut(front_pos).unwrap() = Cell::Body(Body {
                    m_front: front.clone(),
                    m_back: front.get_opposite(),
                });
                self.m_head_pos = front_pos;

                // 生成新的苹果
                self.generate_apple(false);

                Ok(())
            },

            Some(Cell::SuperApple) => {
                *self.get_mut(front_pos).unwrap() = Cell::Body(Body {
                    m_front: front.clone(),
                    m_back: front.get_opposite(),
                });
                self.m_head_pos = front_pos;
                self.m_hid_bodies += 2;

                // 生成新的苹果
                self.generate_apple(false);

                Ok(())
            },

            // 撞到了空气
            Some(Cell::Empty) => {
                // 向后搜索蛇身
                let mut first = true;
                let mut cur_pos = self.m_head_pos;
                let mut bodies = vec![cur_pos];
                loop {
                    let back_pos = {
                        if let Some(Cell::Body(body)) = self.get(cur_pos) {
                            cur_pos + body.m_back.to_distance(1)
                        } else {
                            break;
                        }
                    };
                    if bodies.contains(&back_pos) {
                        break;
                    } else {
                        bodies.push(back_pos);
                    }
                    let back = self.get_mut(back_pos);
                    if back.is_none() {
                        break;
                    }
                    if let Cell::Body(_) = back.unwrap() {
                        cur_pos = back_pos;
                        first = false;
                        continue;
                    } else {
                        break;
                    }
                }
                if self.m_hid_bodies > 0 {
                    self.m_hid_bodies -= 1;
                    *self.get_mut(front_pos).unwrap() = Cell::Body(Body {
                        m_front: front.clone(),
                        m_back: front.get_opposite(),
                    });
                } else if !first {
                    *self.get_mut(cur_pos).unwrap() = Cell::Empty;
                    *self.get_mut(front_pos).unwrap() = Cell::Body(Body {
                        m_front: front.clone(),
                        m_back: front.get_opposite(),
                    });
                } else {
                    self.m_vec.swap(self.m_head_pos.as_1d(self.m_width), front_pos.as_1d(self.m_width));
                }
                self.m_head_pos = front_pos;
                Ok(())
            },

            // 撞到自己了
            Some(Cell::Body(_)) => Err("Snake crashed into itself".to_string()),
            None => Err("Snake crashed into the border".to_string()),
        }
    }
}



pub struct SnakeGame {
    m_map: Map,
    m_width: i16,
    m_height: i16,

    pub config_freeze_screen: bool,
}

impl SnakeGame {
    pub fn new(w: i16, h: i16, len: i16)-> Self {
        execute!(
            stdout(),
            EnterAlternateScreen,
            cursor::Hide,
            //SetSize(w as u16 * 2 + 1, h as u16 + 2),
        ).unwrap();
        enable_raw_mode().unwrap();
        Self {
            m_map: Map::new(w, h, len),
            m_width: w,
            m_height: h,

            config_freeze_screen: true,
        }
    }

    pub fn count_bodies(&self)-> i32 {
        self.m_map.count_bodies()
    }

    pub fn game_loop(&mut self)-> Result<(), String> {
        let draw = |this: &Self| {
            let mut stdout = stdout();
            stdout.queue(BeginSynchronizedUpdate).unwrap();

            queue!(
                stdout,
                MoveTo(0, 0),
                Print({
                    let mut s = "╔".to_string();
                    for _ in 0..this.m_width {
                        s += "══";
                    }
                    s += "╗";
                    s
                }),
            ).unwrap();

            for y in 0..(this.m_height as u16) {
                let mut row = "║".to_string();
                for x in 0..(this.m_width as u16) {
                    let val = this.m_map.get(Position(x as i16, y as i16));
                    match val.unwrap() {
                        Cell::Empty => {
                            row += "  ";
                        },
                        Cell::Body(_) => {
                            row += if this.m_map.m_head_pos == Position(x as i16, y as i16) {
                                "🐍"
                            } else {
                                "🌳"
                            };
                        },
                        Cell::Apple => {
                            row += "🍎";
                        },
                        Cell::SuperApple => {
                            row += "🐔";
                        }
                    }
                }
                row += "║";
                queue!(
                    stdout,
                    MoveTo(0, y as u16 + 1),
                    Print(row),
                ).unwrap()
            }

            queue!(
                stdout,
                MoveTo(0, this.m_height as u16 + 1),
                Print({
                    let mut s = "╚".to_string();
                    for _ in 0..this.m_width {
                        s += "══";
                    }
                    s += "╝";
                    s
                }),
            ).unwrap();

            stdout.queue(EndSynchronizedUpdate).unwrap();

            stdout.flush().unwrap();
        };

        let pause = Arc::new(|| {
        });

        let process_event = |this: &mut Self, e: Event| {
            match e {
                Event::FocusLost => {
                    pause();
                },
                Event::Resize(_, _) => {
                    //stdout().execute(Clear(ClearType::All)).unwrap();
                },
                Event::Key(kevent) => {
                    match kevent.code.clone() {
                        KeyCode::Up => {
                            this.m_map.turn(Direction::Up);
                        },
                        KeyCode::Left => {
                            this.m_map.turn(Direction::Left);
                        },
                        KeyCode::Right => {
                            this.m_map.turn(Direction::Right);
                        },
                        KeyCode::Down => {
                            this.m_map.turn(Direction::Down);
                        },
                        KeyCode::Char(c) => {
                            match c {
                                'Q' | 'q' => {
                                    return true;
                                },
                                _ => {},
                            }
                        }
                        _ => {},
                    }
                },
                _ => {},
            }
            false
        };

        let mut timer = Duration::ZERO;
        loop {
            let begin = Instant::now();
            if let Ok(okay) = event::poll(Duration::from_millis(50)) {
                if okay {
                    if let Ok(event) = event::read() {
                        if process_event(self, event) == true {
                            break;
                        }
                    }
                } else {
                }
            } else {
                break;
            }
            timer += begin.elapsed();

            if timer >= Duration::from_millis(250) {
                self.m_map.update()?;
                timer = Duration::ZERO;
            }

            draw(&self);
        }

        Ok(())
    }
}

impl ops::Drop for SnakeGame {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(
            stdout(),
            cursor::Show,
            LeaveAlternateScreen
        ).unwrap();
    }
}
