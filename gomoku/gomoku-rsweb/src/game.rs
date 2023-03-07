use std::vec::Vec;
use std::ops::*;
use std::mem::swap;
use std::time::{Duration, Instant};
use std::sync::Mutex;

use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub enum Cell {
    Empty,
    Black,
    White,
}

impl Cell {
    pub fn get_reverse(&self)-> Self {
        match *self {
            Cell::Black => Cell::White,
            Cell::White => Cell::Black,
            Cell::Empty => Cell::Empty,
        }
    }

    pub fn reverse(&mut self) {
        *self = self.get_reverse();
    }
}



pub struct Grid {
    m_size: (usize, usize),
    m_vec: Vec<Cell>,
}

impl Grid {
    pub fn new(size: (usize, usize))-> Self {
        let vec_len = size.0 * size.1;
        Self {
            m_size: size.clone(),
            m_vec: vec![Cell::Empty; vec_len],
        }
    }

    pub fn resize(&mut self, size: (usize, usize)) {
        self.m_vec.clone_from(&vec![Cell::Empty; size.0 * size.1]);
    }

    pub fn get(&self, pos: (usize, usize))-> Result<&Cell, String> {
        if pos.0 >= self.m_size.0 || pos.1 >= self.m_size.1 {
            Err(format!("Position ({},{}) is out of range", pos.0, pos.1))
        } else {
            Ok(self.m_vec.index(pos.1 * self.m_size.0 + pos.0))
        }
    }

    pub fn get_mut(&mut self, pos: (usize, usize))-> Result<&mut Cell, String> {
        if pos.0 >= self.m_size.0 || pos.1 >= self.m_size.1 {
            Err(format!("Position ({},{}) is out of range", pos.0, pos.1))
        } else {
            Ok(self.m_vec.index_mut(pos.1 * self.m_size.0 + pos.0))
        }
    }

}



pub struct GameSession {
    m_game_uuid: Uuid,
    m_black_uuid: Uuid,
    m_white_uuid: Uuid,
    m_grid: Grid,
    m_last_activated: Mutex<Instant>,
}

impl GameSession {
    pub fn new()-> Self {
        Self {
            m_game_uuid: Uuid::new_v4(),
            m_black_uuid: Uuid::new_v4(),
            m_white_uuid: Uuid::new_v4(),
            m_grid: Grid::new((15, 15)),
            m_last_activated: Mutex::new(Instant::now()),
        }
    }

    pub fn get_game_uuid(&self)-> Uuid {
        self.m_game_uuid
    }

    pub fn get_white_uuid(&self)-> Uuid {
        self.m_white_uuid
    }

    pub fn get_black_uuid(&self)-> Uuid {
        self.m_black_uuid
    }

    pub fn get_last_activated(&self)-> Instant {
        self.m_last_activated.lock().unwrap().deref().clone()
    }

    pub fn activate(&self) {
        let mut now = Instant::now();
        swap(self.m_last_activated.lock().unwrap().deref_mut(), &mut now);
    }

    pub fn check_timeout(&self)-> bool {
        let elapsed = self.get_last_activated().elapsed();
        elapsed >= Duration::from_secs(600)
    }
}

