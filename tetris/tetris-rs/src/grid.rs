use std::vec::Vec;
use std::ops::*;
use std::default::Default;
use std::sync::{Mutex, RwLock};
use std::cell::Cell as StdCell;

use crossterm::style::{Color};
use lazy_static::lazy_static;
use rand::prelude::*;

pub static BRICK_GRID_SIZE: u16 = 4;



#[derive(Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn new_random()-> Direction {
        let mut rng = thread_rng();
        lazy_static!{
            static ref DS: Vec<Direction> = vec![Direction::Up, Direction::Down, Direction::Right, Direction::Left];
        }
        DS[rng.gen_range(0..DS.len())]
    }

    // 旋转，当传入true时顺时针，否则逆时针
    // 不会更改Direction本身，若要改变，可以使用Direction::rotate_inplace
    pub fn rotate(&self, along_clock: bool)-> Direction {
        if along_clock {
            match *self {
                Direction::Up => Direction::Right,
                Direction::Right => Direction::Down,
                Direction::Down => Direction::Left,
                Direction::Left => Direction::Up,
            }
        } else {
            match *self {
                Direction::Up => Direction::Left,
                Direction::Left => Direction::Down,
                Direction::Down => Direction::Right,
                Direction::Right => Direction::Up,
            }
        }
    }
}

impl Default for Direction {
    fn default()-> Self {
        Self::Up
    }
}





#[derive(Clone)]
pub struct Cell {
    m_block: Option<Block>,
}

impl Cell {
    pub fn new()-> Self {
        Self {
            m_block: None,
        }
    }

    pub fn with_block(bk: Block)-> Self {
        Self {
            m_block: Some(bk),
        }
    }

    /// 检测格子里是否有方块
    pub fn has_block(&self)-> bool {
        self.m_block.is_some()
    }

    /// 清除方块
    pub fn clear(&mut self) {
        self.m_block = None;
    }

    /// 用新的方块替换原来的值
    pub fn replace(&mut self, bk: Block) {
        self.m_block = Some(bk);
    }

    pub fn get(&self)-> &Block {
        self.m_block.as_ref().unwrap()
    }

    pub fn get_mut(&mut self)-> &mut Block {
        self.m_block.as_mut().unwrap()
    }
}

impl Deref for Cell {
    type Target = Block;

    fn deref(&self)-> &Block {
        self.get()
    }
}

impl Default for Cell {
    fn default()-> Self {
        Self::new()
    }
}





#[derive(Clone)]
pub struct Block {
    pub m_color: Color,
}





#[derive(Clone)]
pub struct Grid2D<T> where T: Sized {
    m_vec: Vec<T>,
    m_width: u16,
    m_height: u16,
}

impl<T> Grid2D<T> where T: Sized + Clone {
    pub fn with_value(width: u16, height: u16, value: &T)-> Self {
        let mut obj = Self {
            m_vec: Vec::new(),
            m_width: width,
            m_height: height,
        };
        let nums = width * height;
        for _ in 0..nums {
            obj.m_vec.push(value.clone());
        }
        obj
    }
}

impl<T> Grid2D<T> where T: Sized + Default + Clone {
    pub fn new(width: u16, height: u16)-> Self {
        Self {
            m_vec: vec![T::default(); (width * height) as usize],
            m_width: width,
            m_height: height,
        }
    }
}

impl<T> Grid2D<T> where T: Sized {

    pub fn with_data(width: u16, height: u16, data: Vec<T>)-> Result<Self,String> {
        if (width * height) as usize != data.len() {
            Err("Incorrect data length".to_string())
        } else {
            Ok(Self {
                m_width: width,
                m_height: height,
                m_vec: data,
            })
        }
    }

    /// 获取某一个坐标的值的引用
    pub fn get(&self, x: u16, y: u16)-> Result<&T, String> {
        if x >= self.width() || y >= self.height() {
            Err(format!("Position ({},{}) is out of range", x, y))
        } else {
            let cur = y * self.width() + x;
            Ok(self.m_vec.index((cur) as usize))
        }
    }

    /// 获取某一个坐标的值的可变引用
    pub fn get_mut(&mut self, x: u16, y: u16)-> Result<&mut T, String> {
        if x >= self.width() || y >= self.height() {
            Err(format!("Position ({},{}) is out of range", x, y))
        } else {
            Ok(self.m_vec.index_mut((y * self.width() + x) as usize))
        }
    }

    #[inline]
    pub fn width(&self)-> u16 {
        self.m_width
    }

    #[inline]
    pub fn height(&self)-> u16 {
        self.m_height
    }
}





fn new_brick_grid()-> Grid2D<Cell> {
    Grid2D::<Cell>::new(BRICK_GRID_SIZE, BRICK_GRID_SIZE)
}

#[derive(Clone, Copy)]
pub struct Position(pub i16, pub i16);

pub struct ActiveBrick {
    pub(crate) m_grid_up: Grid2D<Cell>,
    pub(crate) m_grid_down: Grid2D<Cell>,
    pub(crate) m_grid_left: Grid2D<Cell>,
    pub(crate) m_grid_right: Grid2D<Cell>,

    pub(crate) m_direction: Mutex<StdCell<Direction>>,

    pub(crate) m_switch_lock: RwLock<()>,
}

impl ActiveBrick {
    pub fn new()-> Self {
        let meta = new_brick_grid();
        Self {
            m_grid_down: meta.clone(),
            m_grid_left: meta.clone(),
            m_grid_right: meta.clone(),
            m_grid_up: meta,

            m_direction: Mutex::new(StdCell::new(Direction::Up)),

            m_switch_lock: RwLock::new(()),
        }
    }

    pub fn get_active_grid(&self)-> &Grid2D<Cell> {
        self.get_grid(self.direction())
    }

    pub fn get_mut_active_grid(&mut self)-> &Grid2D<Cell> {
        self.get_mut_grid(self.direction())
    }

    pub fn get_grid(&self, d: Direction)-> &Grid2D<Cell> {
        match d {
            Direction::Up => &self.m_grid_up,
            Direction::Down => &self.m_grid_down,
            Direction::Left => &self.m_grid_left,
            Direction::Right => &self.m_grid_right,
        }
    }

    pub fn get_mut_grid(&mut self, d: Direction)-> &mut Grid2D<Cell> {
        match d {
            Direction::Up => &mut self.m_grid_up,
            Direction::Down => &mut self.m_grid_down,
            Direction::Left => &mut self.m_grid_left,
            Direction::Right => &mut self.m_grid_right,
        }
    }

    pub fn switch(&self, d: Direction) {
        let _lock = self.m_switch_lock.write().unwrap();
        self.m_direction.lock().unwrap().set(d);
    }

    pub fn direction(&self)-> Direction {
        self.m_direction.lock().unwrap().get()
    }

    /// 获取向某个方向移动时需要检测的点
    pub fn get_checking_points(&self, d: Direction)-> Vec<Position> {
        let _lock = self.m_switch_lock.read().unwrap();
        let mut v = Vec::<Position>::with_capacity(10);
        let grid = self.get_active_grid();
        let g_w = grid.width();
        let g_h = grid.height();
        match d {
            Direction::Up => {
                for x in 0..g_w {
                    for y in 0..g_h {
                        if grid.get(x, y).unwrap().has_block() {
                            v.push(Position(x as i16, y as i16 - 1));
                            break;
                        }
                    }
                }
            },
            Direction::Down => {
                for x in 0..g_w {
                    for y in 1..=g_h {
                        let y = g_h - y;
                        if grid.get(x, y).unwrap().has_block() {
                            v.push(Position(x as i16, y as i16 + 1));
                            break;
                        }
                    }
                }
            },
            Direction::Left => {
                for y in 0..g_h {
                    for x in 0..g_w {
                        if grid.get(x, y).unwrap().has_block() {
                            v.push(Position(x as i16 - 1, y as i16));
                            break;
                        }
                    }
                }
            },
            Direction::Right => {
                for y in 0..g_h {
                    for x in 1..=g_w {
                        let x = g_w - x;
                        if grid.get(x, y).unwrap().has_block() {
                            v.push(Position(x as i16 + 1, y as i16));
                            break;
                        }
                    }
                }
            },
        }
        v
    }

    /*pub fn get_rotating_checking_points(&self)-> Vec<Position> {
    }*/

    pub fn get_active_content_width(&self)-> u16 {
        let _lock = self.m_switch_lock.read().unwrap();
        let grid = self.get_active_grid();
        let mut first = 0u16;
        let mut last = 0u16;
        let mut empty = true;
        for x in 0..grid.width() {
            for y in 0..grid.height() {
                if grid.get(x, y).unwrap().has_block() {
                    if first == 0 {
                        first = x;
                    }
                    last = x;
                    empty = false;
                }
            }
        }
        if empty {
            0
        } else {
            last - first + 1
        }
    }

    pub fn get_active_content_height(&self)-> u16 {
        let _lock = self.m_switch_lock.read().unwrap();
        let grid = self.get_active_grid();
        let mut first = 0u16;
        let mut last = 0u16;
        let mut empty = true;
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                if grid.get(x, y).unwrap().has_block() {
                    if first == 0 {
                        first = y;
                    }
                    last = y;
                    empty = false;
                }
            }
        }
        if empty {
            0
        } else {
            last - first + 1
        }
    }
}

impl Clone for ActiveBrick {
    fn clone(&self)-> Self {
        Self {
            m_grid_up: self.m_grid_up.clone(),
            m_grid_down: self.m_grid_down.clone(),
            m_grid_left: self.m_grid_left.clone(),
            m_grid_right: self.m_grid_right.clone(),
            m_direction: Mutex::new(StdCell::new(self.m_direction.lock().unwrap().get())),
            m_switch_lock: RwLock::new(()),
        }
    }
}



pub fn random_brick()-> ActiveBrick {
    macro_rules! put_mb {
        ($(($x:expr, $y:expr)),* -> ($grid_obj:expr)($meta_block:expr)) => {
            $( ($grid_obj).get_mut($x, $y).unwrap().replace(($meta_block).clone()); )*
        }
    }
    macro_rules! make_brick {
        ($up:expr, $down:expr, $right:expr, $left:expr, $d:expr) => {
            ActiveBrick {
                m_grid_up: $up,
                m_grid_down: $down,
                m_grid_left: $left,
                m_grid_right: $right,
                m_direction: Mutex::new(StdCell::new($d)),
                m_switch_lock: RwLock::new(()),
            }
        }
    }
    lazy_static! {static ref COLORS: Vec<Color> = vec![
        Color::Yellow,
        Color::Green,
        Color::Red,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Grey,
    ];}
    let mut rng = rand::thread_rng();
    let mb = Block {m_color: COLORS[rng.gen_range(0..(COLORS.len()))].clone()};
    match rng.gen_range(0..7) {
    //match 0 {
        // 方形方块
        0 => {
            let mut grid = new_brick_grid();
            put_mb!(
                (0, 0), (1, 0),
                (0, 1), (1, 1) -> (grid)(mb)
            );
            make_brick!(grid.clone(),grid.clone(),grid.clone(),grid.clone(),Direction::Up)
        },
        // T形
        1 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();
            put_mb!(
                        (1, 0),
                (0, 1), (1, 1), (2, 1) -> (g_up)(mb)
            );
            put_mb!(
                (0, 0), (1, 0), (2, 0),
                        (1, 1) -> (g_down)(mb)
            );
            put_mb!(
                        (1, 0),
                (0, 1), (1, 1),
                        (1, 2) -> (g_left)(mb)
            );
            put_mb!(
                        (1, 0),
                        (1, 1), (2, 1),
                        (1, 2) -> (g_right)(mb)
            );

            make_brick!(g_up,g_down,g_right,g_left,Direction::new_random())
        },
        // 反L形
        2 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();
            put_mb!(
                        (1, 0),
                        (1, 1),
                (0, 2), (1, 2) -> (g_left)(mb)
            );

            put_mb!(
                (0, 0),
                (0, 1), (1, 1), (2, 1) -> (g_up)(mb)
            );

            put_mb!(
                        (1, 0), (2, 0),
                        (1, 1),
                        (1, 2) -> (g_right)(mb)
            );

            put_mb!(
                (0, 1), (1, 1), (2, 1),
                                (2, 2) -> (g_down)(mb)
            );

            make_brick!(g_up,g_down,g_right,g_left,Direction::new_random())
        },
        // L形
        3 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();
            put_mb!(
                (0, 0), (1, 0),
                        (1, 1),
                        (1, 2) -> (g_left)(mb)
            );

            put_mb!(
                (0, 1), (1, 1), (2, 1),
                (0, 2) -> (g_down)(mb)
            );

            put_mb!(
                        (1, 0),
                        (1, 1),
                        (1, 2), (2, 2) -> (g_right)(mb)
            );

            put_mb!(
                                (2, 0),
                (0, 1), (1, 1), (2, 1) -> (g_up)(mb)
            );

            make_brick!(g_up,g_down,g_right,g_left,Direction::new_random())
        },
        // 长条
        4 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();
            put_mb!{
                (1, 0),
                (1, 1),
                (1, 2),
                (1, 3) -> (g_right)(mb)
            }

            put_mb!{
                (0, 2), (1, 2), (2, 2), (3, 2) -> (g_up)(mb)
            }

            put_mb!{
                (0, 1), (1, 1), (2, 1), (3, 1) -> (g_down)(mb)
            }

            put_mb!{
                (2, 0),
                (2, 1),
                (2, 2),
                (2, 3) -> (g_left)(mb)
            }

            make_brick!(g_up,g_down,g_right,g_left,Direction::new_random())
        },
        // S形
        5 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();

            put_mb!{
                        (1, 0), (2, 0),
                (0, 1), (1, 1) -> (g_up)(mb)
            }

            put_mb!{
                (0, 0),
                (0, 1), (1, 1),
                        (1, 2) -> (g_left)(mb)
            }

            put_mb!{
                        (1, 0), (2, 0),
                (0, 1), (1, 1) -> (g_down)(mb)
            }

            put_mb!{
                (0, 0),
                (0, 1), (1, 1),
                        (1, 2) -> (g_right)(mb)
            }

            make_brick!(g_up,g_down,g_right,g_left,Direction::new_random())
        },
        // Z形
        6 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();

            put_mb!{
                (0, 0), (1, 0),
                        (1, 1), (2, 1) -> (g_up)(mb)
            }

            put_mb!{
                        (1, 0),
                (0, 1), (1, 1),
                (0, 2) -> (g_left)(mb)
            }

            put_mb!{
                (0, 0), (1, 0),
                        (1, 1), (2, 1) -> (g_down)(mb)
            }

            put_mb!{
                        (1, 0),
                (0, 1), (1, 1),
                (0, 2) -> (g_right)(mb)
            }

            make_brick!(g_up,g_down,g_right,g_left,Direction::new_random())
        },
        7 => {
            let mut g_up = new_brick_grid();
            let mut g_down = new_brick_grid();
            let mut g_left = new_brick_grid();
            let mut g_right = new_brick_grid();

            put_mb!{
                (0, 0),
                        (1, 1),
                                (2, 2) -> (g_up)(mb)
            }

            put_mb!{
                                (2, 0),
                        (1, 1),
                (0, 2) -> (g_right)(mb)
            }

            put_mb!{
                (0, 0),
                        (1, 1),
                                (2, 2) -> (g_down)(mb)
            }

            put_mb!{
                (0, 0), (1, 0), (2, 0),
                (0, 1),         (2, 1),
                (0, 2), (1, 2), (2, 2) -> (g_left)(mb)
            }

            make_brick!(g_left.clone(),g_left.clone(),g_left.clone(),g_left.clone(),Direction::new_random())
        },
        _ => unreachable!(),
    }
}


