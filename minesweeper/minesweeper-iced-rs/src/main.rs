use iced::widget::canvas::{
    Cursor,
    Frame,
    Geometry,
    Path,
    Program,
    Stroke,
    Style,
    Text,
    Event as CanvasEvent,
};
use iced::{
    alignment::*,
    mouse::Event as MouseEvent,
    mouse::Button as MouseButton,
    event::Status as EventStatus,
    Element,
    Font,
    Settings,
    Length,
    Theme,
    Command,
    Renderer,
    Rectangle,
    Size,
    Point,
    Padding,
    Color,
};
use iced::application::Application;
use iced::widget::*;
use iced::window as win;
use vec2d::{ Vec2D, Coord };
use rand::prelude::*;
use once_cell::sync::OnceCell;

use std::sync::Mutex;
use std::collections::VecDeque;

#[derive(Copy, Clone, Debug)]
enum Message {
    Gameover,
    Nothing,
}

#[derive(Clone, Debug)]
struct Cell {
    pub has_mine: bool,
    pub opened: bool,
    pub mines_counter: i32,
    pub marked: bool,
}

struct Battleground;

impl BattlegroundState {
    fn canvas2coord(&self, bounds: Rectangle<f32>, mut point: Point)-> Option<Coord> {
        let size = bounds.size();
        if !bounds.contains(point) {
            return None;
        } else {
            point.x -= bounds.x;
            point.y -= bounds.y;
        }
        let m_vec = self.m_vec.lock().unwrap();
        let width = size.width / m_vec.size().width as f32;
        let height = size.height / m_vec.size().height as f32;

        let x = (point.x / width) as usize;
        let y = (point.y / height) as usize;

        Some(Coord::new(x, y))
    }

    /// 随机生成指定数量的雷，指定一个点以排除这个点及其周围的8个点生成雷的可能性
    pub fn place_mines(m_vec: &mut std::sync::MutexGuard<'_, Vec2D<Cell>>, number: usize, exclude: (usize, usize))-> Result<(), ()> {
        let offsets: Vec<(isize, isize)> = vec![
            (-1, -1), (-1, 0), (-1, 1), (0, -1),
            (0, 1), (1, -1), (1, 0), (1, 1)
        ];
        //let mut m_vec = self.m_vec.lock().unwrap();
        let mut rng = rand::thread_rng();
        let size = m_vec.size();

        if size.width * size.height - 9 < number {
            return Err(());
        }

        let is_near = |mut a: usize, mut b: usize|-> bool {
            if a < b {
                let tmp = a;
                a = b;
                b = tmp;
            }

            a - b <= 1
        };
        for _ in 0..number {
            let mut cell;
            let point = loop {
                let x = rng.gen_range(0..size.width);
                let y = rng.gen_range(0..size.height);

                if is_near(exclude.0, x) && is_near(exclude.1, y) {
                    continue;
                } else {
                    let point = Coord::new(x, y);
                    cell = m_vec.get_mut(point).unwrap();
                    if cell.has_mine {
                        continue;
                    } else {
                        break point;
                    }
                }
            };

            let mut cell = m_vec.get_mut(point).unwrap();
            cell.has_mine = true;
            drop(cell);
            offsets.iter().map(|i| Coord::new(
                (point.x as isize - i.0) as usize,
                (point.y as isize - i.1) as usize
            )).for_each(|i| {
                if let Some(cell) = m_vec.get_mut(i) {
                    cell.mines_counter += 1;
                }
            });
        }

        Ok(())
    }

    /// 将所有格子置为未打开状态
    /*
    pub fn close_all(&self) {
        self.m_vec.lock().unwrap().iter_mut().for_each(|i| i.1.opened = false);
    }
    */

    /// 清除所有雷
    /*
    pub fn clear_all_mines(&self) {
        self.m_vec.lock().unwrap().iter_mut().for_each(|i| {
            i.1.has_mine = false;
            i.1.mines_counter = 0;
        });
    }
    */

    pub fn left_click(&mut self, coord: Coord)-> Message {
        let offsets: Vec<(isize, isize)> = vec![
            (-1, -1), (-1, 0), (-1, 1), (0, -1),
            (0, 1), (1, -1), (1, 0), (1, 1)
        ];
        let mut queue = VecDeque::<Coord>::from(vec![coord]);
        let mut m_vec = self.m_vec.lock().unwrap();
        if m_vec.get(coord).unwrap().opened {
            return Message::Nothing;
        }

        while !queue.is_empty() {
            let current_coord = queue.pop_front().unwrap();

            let target = m_vec.get_mut(current_coord);

            if let Some(current_cell) = target {
                if current_cell.has_mine && !current_cell.marked {
                    return Message::Gameover;
                }
                if !current_cell.opened && !current_cell.marked {
                    let current_cell = if !self.generated {
                        drop(current_cell);
                        Self::place_mines(&mut m_vec, *MINES.wait(), (coord.x, coord.y)).expect("Cannot place mines");
                        self.generated = true;
                        m_vec.get_mut(current_coord).unwrap()
                    } else {
                        current_cell
                    };
                    current_cell.opened = true;
                    if current_cell.mines_counter == 0 {
                        offsets.iter().map(|i| Coord::new(
                            (current_coord.x as isize + i.0) as usize,
                            (current_coord.y as isize + i.1) as usize,
                        )).for_each(|i| queue.push_back(i));
                    }
                }
            } else {
                continue;
            }
        }

        Message::Nothing
    }

    pub fn right_click(&self, coord: Coord)-> Message {
        let mut m_vec = self.m_vec.lock().unwrap();

        if let Some(current_cell) = m_vec.get_mut(coord) {
            if !current_cell.opened {
                current_cell.marked = !current_cell.marked;
            }
        }

        Message::Nothing
    }
}

#[derive(Debug)]
struct BattlegroundState {
    pub m_vec: Mutex<Vec2D<Cell>>,
    pub left_pressed: Option<Coord>,
    pub right_pressed: Option<Coord>,

    pub generated: bool,
}
impl Default for BattlegroundState {
    fn default()-> Self {
        Self {
            m_vec: Mutex::new(Vec2D::from_example(vec2d::Size::new(*SIZE.wait(), *SIZE.wait()), &Cell {
                opened: false,
                has_mine: false,
                mines_counter: 0,
                marked: false,
            })),
            left_pressed: None,
            right_pressed: None,
            generated: false,
        }
    }
}

impl Program<Message> for Battleground {
    type State = BattlegroundState;

    fn draw(&self,
        state: &Self::State,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor
    )-> Vec<Geometry> {
        let m_vec = state.m_vec.lock().unwrap();
        let mut frame = Frame::new(bounds.size());
        let stroke = Stroke {
            style: Style::Solid(Color::BLACK),
            width: bounds.size().width / 180.0,
            ..Default::default()
        };

        let vec_size = m_vec.size();
        let cell_width = bounds.size().width / vec_size.width as f32;
        let cell_height = bounds.size().height / vec_size.height as f32;

        for x in 0..vec_size.width {
            for y in 0..vec_size.height {
                let point = Point::new(cell_width * x as f32, cell_height * y as f32);
                let central_point = Point::new(point.x + cell_width / 2.0, point.y + cell_height / 2.0);
                let box_ = Path::rectangle(point, Size::new(cell_width, cell_height));

                let cell = m_vec.get(Coord::new(x, y)).unwrap();
                if cell.opened {
                    //println!("({},{}) Opened", x, y);
                    if cell.has_mine {
                        frame.fill_text(Text {
                            content: "M".to_string(),
                            position: central_point,
                            color: Color::BLACK,
                            size: cell_height,
                            font: Font::Default,
                            horizontal_alignment: Horizontal::Center,
                            vertical_alignment: Vertical::Center,
                        });
                    } else if cell.mines_counter > 0 {
                        frame.fill_text(Text {
                            content: cell.mines_counter.to_string(),
                            position: central_point,
                            color: number_to_color(cell.mines_counter),
                            size: cell_height,
                            font: Font::Default,
                            horizontal_alignment: Horizontal::Center,
                            vertical_alignment: Vertical::Center,
                        });
                    }
                } else {
                    let x_padding = cell_width * 0.1;
                    let y_padding = cell_height * 0.1;
                    let inner_box = Path::rectangle(
                        Point::new(
                            point.x + x_padding,
                            point.y + y_padding
                        ),
                        Size::new(
                            cell_width * 0.8,
                            cell_height * 0.8
                        )
                    );
                    frame.fill(&inner_box, Color::from_rgb(0.3, 0.3, 0.3));
                }

                frame.stroke(&box_, stroke.clone());

                if cell.marked {
                    let stick = Path::rectangle(
                        Point::new(
                            point.x + cell_width * 0.45,
                            point.y + cell_height * 0.24
                        ),
                        Size::new(
                            cell_width * 0.06,
                            cell_width * 0.6
                        )
                    );

                    frame.fill(&stick, Color::from_rgb(0.42, 0.12, 0.00));

                    let flag = Path::new(|b| {
                        b.move_to(Point::new(point.x + cell_width * 0.51, point.y + cell_height * 0.25));
                        b.line_to(Point::new(point.x + cell_width * 0.51, point.y + cell_height * 0.48));
                        b.line_to(Point::new(point.x + cell_width * 0.78, point.y + cell_height * 0.34));
                        b.close();
                    });

                    frame.fill(&flag, Color::from_rgb(0.82, 0.16, 0.23));
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(&self,
        state: &mut Self::State,
        event: CanvasEvent,
        bounds: Rectangle<f32>,
        cursor: Cursor
    )-> (EventStatus, Option<Message>) {
        let pointer = if let Cursor::Available(pointer) = cursor {
            pointer
        } else {
            return (EventStatus::Ignored, None);
        };
        match event {
            CanvasEvent::Mouse(mouse) => {
                match mouse {
                    MouseEvent::ButtonPressed(btn) => {
                        match btn {
                            MouseButton::Left => {
                                state.left_pressed = state.canvas2coord(bounds, pointer);
                            },
                            MouseButton::Right => {
                                state.right_pressed = state.canvas2coord(bounds, pointer);
                            },
                            _ => {},
                        }
                    },
                    MouseEvent::ButtonReleased(btn) => {
                        let coord = state.canvas2coord(bounds, pointer);
                        if coord.is_some() {
                            match btn {
                                MouseButton::Left if state.left_pressed == coord => {
                                    return (EventStatus::Captured, Some(state.left_click(coord.unwrap())));
                                },
                                MouseButton::Right if state.right_pressed == coord => {
                                    
                                    return (EventStatus::Captured, Some(state.right_click(coord.unwrap())));
                                },
                                _ => {},
                            }
                        }
                        state.right_pressed = None;
                        state.left_pressed = None;
                    },
                    _ => {},
                }
            },

            _ => {},
        }

        (EventStatus::Captured, None)
    }
}

impl Default for Battleground {
    fn default()-> Self {
        Battleground
    }
}

struct MineSweeper {
    pub size: (u32, u32),
}

#[allow(unused_parens)]
impl Application for MineSweeper {

    type Executor = iced::executor::Default;
    type Theme = Theme;
    type Flags = ((u32, u32));
    type Message = Message;

    fn new(flags: Self::Flags)-> (Self, Command<Self::Message>) {
        (Self {
            size: flags,
        }, Command::none())
    }

    fn title(&self)-> String {
        "MineSweeper".to_string()
    }

    fn update(&mut self, msg: Self::Message)-> Command<Self::Message> {
        match msg {
            Message::Gameover => {
                win::close()
            },

            _ => {
                Command::none()
            },
        }
    }

    fn view(&self)-> Element<'_, Self::Message, Renderer<Self::Theme>> {
        let size = (self.size.0 as f32, self.size.1 as f32);
        let canvas = Canvas::new(Battleground)
            .height(Length::Fixed(size.1 * 0.84))
            .width(Length::Fixed(size.1 * 0.84));
        let pad = Padding {
            top: size.1 * 0.08,
            bottom: size.1 * 0.08,
            left: size.0 * 0.04,
            right: size.0 * 0.04,
        };
        Row::new()
            .padding(pad)
            .push(canvas)
            .into()
    }

}

fn get_window_size()-> (u32, u32) {
    (800, 500)
}

fn number_to_color(number: i32)-> Color {
    match number {
        1 => Color::new(0.08, 0.25, 1.00, 1.00), // 亮蓝色
        2 => Color::new(0.00, 0.64, 0.06, 1.00), // 灰绿色
        3 => Color::new(0.92, 0.06, 0.06, 1.00), // 亮红色
        4 => Color::new(0.10, 0.34, 0.59, 1.00), // 暗蓝色
        5 => Color::new(0.58, 0.07, 0.08, 1.00), // 暗红色
        6 => Color::new(0.07, 0.71, 0.73, 1.00), // 青色
        7 => Color::new(0.10, 0.10, 0.10, 1.00), // 黑色
        8 => Color::new(0.75, 0.75, 0.75, 1.00), // 灰色
        _ => Color::BLACK,
    }
}

fn init_args() {
    let args = std::env::args();

    let difficulty = OnceCell::new();
    let mut last = None::<String>;

    for arg in args {
        if let Some(v) = last.clone() {
            match &v[..] {
                "-size" => {
                    SIZE.set(arg.parse::<usize>().unwrap()).unwrap();
                },
                "-difficulty" => {
                    difficulty.set(arg.parse::<f64>().unwrap()).unwrap();
                },
                _ => {
                    panic!("Unrecognized option");
                },
            }
            last = None;
        }
        if arg.starts_with("-") {
            last = Some(arg);
        }
    }

    if let Some(_) = last {
        panic!("A option has empty value");
    }

    let size = SIZE.get_or_init(|| 9);
    let difficulty = difficulty.get_or_init(|| 0.2);

    MINES.set(((size * size) as f64 * difficulty) as usize).unwrap();
}

static MINES: OnceCell<usize> = OnceCell::new();
static SIZE: OnceCell<usize> = OnceCell::new();

fn main()-> iced::Result {
    init_args();

    MineSweeper::run(Settings {
        window: win::Settings {
            position: win::Position::Centered,
            size: get_window_size(),
            resizable: false,
            ..Default::default()
        },
        flags: get_window_size(),
        ..Default::default()
    })
}
