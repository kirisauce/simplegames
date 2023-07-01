use bevy::prelude::*;
use bevy::window::*;
use bevy::winit::{ WinitSettings, UpdateMode };
use bevy::sprite::{ MaterialMesh2dBundle, Mesh2dHandle };
use bevy::render::mesh::PrimitiveTopology;

use std::time::{ Duration, Instant };
use std::sync::Mutex;

const CHESS_NORMAL_COLOR: Color = Color::rgb(1., 0.92, 0.63);
const CHESS_HOVERED_COLOR: Color = Color::rgb(1., 0.96, 0.82);
const PREVIEW_POINT_COLOR: Color = Color::rgb(0.65, 1., 0.73);

#[derive(Default, Copy, Clone, Eq, PartialEq, States, Debug, Hash)]
enum AppState {
    #[default]
    Ingame,

    MainMenu,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Team {
    Red,
    Black,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    /// 帅 & 将
    King,

    /// 仕 & 士
    Guard,

    /// 炮 & 砲
    Cannon,

    /// 相 & 象
    Bishop(bool),

    /// 马
    Horse,

    /// 兵 & 卒
    /// `false`代表棋盘上方的兵(卒), `true`代表棋盘下方的兵(卒)
    Pawn(bool),

    /// 车
    Chariot,
}

#[derive(Debug, Clone)]
struct HistoryRecord {
    pub from_pos: (i32, i32),
    pub to_pos: (i32, i32),
    pub target_chess: Option<Chess>,
}

/// 表示当前实体是一个棋子
#[derive(Component, Debug, Clone)]
struct Chess {
    team: Team,
    role: Role,
    position: (i32, i32),
    redraw_stage: u8,
}

/// 表示当前棋子被选中了
#[derive(Component, Debug)]
struct Selected;

/// 窗口大小
#[derive(Resource, Clone, Copy, Debug)]
struct WindowSize(pub f32, pub f32);

#[derive(Resource, PartialEq, Eq)]
struct CurrentTeam(Team);

#[derive(Resource, Debug, Deref, DerefMut, Default)]
struct History(Vec<HistoryRecord>);

/// 游戏中的实体
#[derive(Component)]
struct Ingame;

#[derive(Component)]
struct UndoButton;

/// 棋盘实体
#[derive(Component)]
struct Chessboard;

/// 棋盘按钮组件
#[derive(Component)]
struct ChessButton {
    x: i32,
    y: i32,
}

#[derive(Component)]
struct TeamSuggestion(Team);

#[derive(Component, Debug)]
struct TransformAnimation {
    pub begin_time: Instant,
    pub duration: Duration,
    pub begin_state: Transform,
    pub end_state: Transform,
    pub timing_function: fn(f32)-> f32,
    pub activated: bool,
}

#[derive(Component, PartialEq, Eq)]
struct PreviewPoint(i32, i32);

impl WindowSize {
    pub fn compute_width(&self)-> f32 {
        self.0 * 0.88 - self.compute_button_size()
    }

    pub fn compute_height(&self)-> f32 {
        self.compute_width() * 0.88 * 1.245 - self.compute_button_size()
    }

    pub fn compute_button_size(&self)-> f32 {
        self.0 * 0.88 / 9. - 10.
    }

    pub fn compute_padding(&self)-> f32 {
        self.compute_button_size() / 2. + self.0 * 0.06
    }
}

impl Chess {
    pub fn to_owned_string(&self)-> String {
        self.to_string().to_owned()
    }

    pub fn to_string(&self)-> &'static str {
        let pair = match self.role {
            Role::King      => ("帅", "将"),
            Role::Guard     => ("仕", "士"),
            Role::Cannon    => ("炮", "砲"),
            Role::Pawn(_)   => ("兵", "卒"),
            Role::Bishop(_) => ("相", "象"),
            Role::Horse     => ("馬", "馬"),
            Role::Chariot   => ("車", "車"),
        };
        match self.team {
            Team::Red   => pair.0,
            Team::Black => pair.1,
        }
    }

    pub fn get_color(&self)-> Color {
        match self.team {
            Team::Red => Color::rgb(0.95, 0.2, 0.2),
            Team::Black => Color::rgb(0.1, 0.1, 0.1),
        }
    }
}

impl Default for CurrentTeam {
    fn default()-> Self {
        Self(Team::Red)
    }
}

impl Default for WindowSize {
    fn default()-> Self {
        Self(0., 0.)
    }
}

impl Team {
    fn opposite(&self)-> Self {
        match *self {
            Team::Red => Team::Black,
            Team::Black => Team::Red,
        }
    }
}

impl TransformAnimation {
    pub fn get_progress(&self)-> f32 {
        if self.is_done() {
            1.
        } else {
            let elapsed = self.begin_time.elapsed();
            elapsed.as_millis() as f32 / self.duration.as_millis() as f32
        }
    }

    pub fn is_done(&self)-> bool {
        self.begin_time.elapsed() >= self.duration || !self.activated
    }

    pub fn transform(&self)-> Transform {
        let t1 = self.begin_state;
        let t2 = self.end_state;
        let p = self.get_progress();
        Transform {
            translation: self.trans_vec3(t1.translation, t2.translation, p),
            rotation: t2.rotation,
            scale: self.trans_vec3(t1.scale, t2.scale, p),
        }
    }

    fn trans_vec3(&self, v1: Vec3, v2: Vec3, progress: f32)-> Vec3 {
        Vec3 {
            x: self.trans_point(v1.x, v2.x, progress),
            y: self.trans_point(v1.y, v2.y, progress),
            z: self.trans_point(v1.z, v2.z, progress),
        }
    }

    fn trans_point(&self, p1: f32, p2: f32, progress: f32)-> f32 {
        p1 + (p2 - p1) * (self.timing_function)(progress)
    }

    pub fn init_time(&mut self, duration: Duration) {
        self.begin_time = Instant::now();
        self.duration = duration;
    }

    pub fn activate(&mut self) {
        self.activated = true;
    }

    pub fn unactivate(&mut self) {
        self.activated = false;
    }
}

impl Default for TransformAnimation {
    fn default()-> Self {
        fn linear(p: f32)-> f32 {
            p
        }
        Self {
            begin_time: Instant::now(),
            duration: Duration::ZERO,
            begin_state: Transform::default(),
            end_state: Transform::default(),
            timing_function: linear,
            activated: false,
        }
    }
}



fn main() {
    let winit_settings = if let Some(m) = std::env::args().nth(1) {
        if &m[..] == "--save-power" {
            WinitSettings {
                focused_mode: UpdateMode::Reactive {
                    max_wait: Duration::from_millis(80),
                },
                unfocused_mode: UpdateMode::ReactiveLowPower {
                    max_wait: Duration::from_millis(200),
                },
                ..Default::default()
            }
        } else {
            WinitSettings::default()
        }
    } else {
        WinitSettings::default()
    };

    App::new()
        .add_plugins(DefaultPlugins)

        .insert_resource(winit_settings)

        .add_startup_system(game_setup_system)
        .add_system(window_size_update_system)
        .add_system(team_suggestion_system)
        .add_system(transform_animation_system)
        .add_systems((game_system, chessboard_system).after(window_size_update_system))

        .run()
}

/// 将屏幕坐标系转换为Bevy所使用的中央坐标系
fn screen_to_bevy(window_size: WindowSize, position: (f32, f32))-> (f32, f32) {
    (position.0 - window_size.0 / 2., window_size.1 / 2. - position.1)
}

/// 创建棋盘背景的Mesh
fn create_chessboard_mesh(window_size: WindowSize)-> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList);
    let mut points = Vec::new();
    let width = window_size.compute_width();
    let height = window_size.compute_height();
    let padding = window_size.compute_padding();

    let mut add_line = |start: (f32, f32), end: (f32, f32)| {
        points.push([start.0, start.1, 0.]);
        points.push([end.0,   end.1,   0.]);
    };

    for x in 0..9 {
        let x_pos = padding + width * (x as f32 / 8.);

        let start0   = screen_to_bevy(window_size,
            (x_pos, padding));
        let mut end0 = screen_to_bevy(window_size,
            (x_pos, padding + height * 0.445));

        if x != 0 && x != 8 {
            // 不是边缘的线则在中间留一格空(楚河 汉界)
            let start1 = screen_to_bevy(window_size,
                (x_pos, padding + height * 0.555));
            let end1   = screen_to_bevy(window_size,
                (x_pos, padding + height));

            add_line(start1, end1);
        } else {
            // 如果是边缘的线则不在中间留空一格
            end0 = screen_to_bevy(window_size,
                (x_pos, padding + height));
        }

        add_line(start0, end0);
    }

    for y in 0..10 {
        let y_pos = padding + height * (y as f32 / 9.);

        let start = screen_to_bevy(window_size,
            (padding, y_pos));
        let end   = screen_to_bevy(window_size,
            (padding + width, y_pos));

        add_line(start, end);
    }

    // 宫格
    {
        // 左上-右下的斜线
        let mut start0 = screen_to_bevy(window_size,
            (padding + width * 0.375, padding));

        let mut end0   = screen_to_bevy(window_size,
            (padding + width * 0.625, padding + height * 0.222));

        // 右上-左下的斜线
        let mut start1 = screen_to_bevy(window_size,
            (padding + width * 0.625, padding));

        let mut end1   = screen_to_bevy(window_size,
            (padding + width * 0.375, padding + height * 0.222));

        add_line(start0, end0);
        add_line(start1, end1);

        for i in vec![&mut start0, &mut end0, &mut start1, &mut end1] {
            i.1 -= height * 0.777;
        }

        add_line(start0, end0);
        add_line(start1, end1);
    }

    // 兵 & 炮位标识
    {
        let positions = vec![
            (1., 2.), (7., 2.),
            (0., 3.), (2., 3.), (4., 3.), (6., 3.), (8., 3.),
            (0., 6.), (2., 6.), (4., 6.), (6., 6.), (8., 6.),
            (1., 7.), (7., 7.)
        ];
        let distance_short = width * 0.015;
        let distance_long  = width * 0.032;

        for position in positions {

            let screen_pos = (
                padding + width  * position.0 / 8.,
                padding + height * position.1 / 9.
            );

            if position.0 as i32 != 0 {
                // 左上
                let point0 = screen_to_bevy(window_size,
                    (screen_pos.0 - distance_long, screen_pos.1 - distance_short));
                let point1 = screen_to_bevy(window_size,
                    (screen_pos.0 - distance_short, screen_pos.1 - distance_short));
                let point2 = screen_to_bevy(window_size,
                    (screen_pos.0 - distance_short, screen_pos.1 - distance_long));

                add_line(point0, point1);
                add_line(point1, point2);

                // 左下
                let point0 = screen_to_bevy(window_size,
                    (screen_pos.0 - distance_long, screen_pos.1 + distance_short));
                let point1 = screen_to_bevy(window_size,
                    (screen_pos.0 - distance_short, screen_pos.1 + distance_short));
                let point2 = screen_to_bevy(window_size,
                    (screen_pos.0 - distance_short, screen_pos.1 + distance_long));

                add_line(point0, point1);
                add_line(point1, point2);
            }

            if position.0 as i32 != 8 {
                // 右上
                let point0 = screen_to_bevy(window_size,
                    (screen_pos.0 + distance_long, screen_pos.1 - distance_short));
                let point1 = screen_to_bevy(window_size,
                    (screen_pos.0 + distance_short, screen_pos.1 - distance_short));
                let point2 = screen_to_bevy(window_size,
                    (screen_pos.0 + distance_short, screen_pos.1 - distance_long));

                add_line(point0, point1);
                add_line(point1, point2);

                // 右下
                let point0 = screen_to_bevy(window_size,
                    (screen_pos.0 + distance_long, screen_pos.1 + distance_short));
                let point1 = screen_to_bevy(window_size,
                    (screen_pos.0 + distance_short, screen_pos.1 + distance_short));
                let point2 = screen_to_bevy(window_size,
                    (screen_pos.0 + distance_short, screen_pos.1 + distance_long));

                add_line(point0, point1);
                add_line(point1, point2);
            }
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, points);

    mesh
}

/// 创建棋子的Mesh
fn create_chess_mesh(window_size: WindowSize)-> Mesh {
    let mesh = shape::Circle::new((window_size.0 * 0.88 - 90.) / 9. * 0.52);
    mesh.into()
}

/// 创建预览点的Mesh
fn create_preview_point_mesh(window_size: WindowSize)-> Mesh {
    let mesh = shape::Circle::new((window_size.0 * 0.88 - 90.) / 9. * 0.17);
    mesh.into()
}

/// 创建队伍提示的Mesh
fn create_team_suggestion_mesh(window_size: WindowSize, team: Team, activated: bool)-> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    let mut vertices = Vec::new();
    //let mut vertice_colors = Vec::new();
    let w = window_size.0;
    let rect_h = w * 0.135;

    macro_rules! add_points {
        ($($position:expr),*) => {
            $({
            let p = $position;
            vertices.push([p.0, p.1, 0.]);
            //vertice_colors.push(($color).as_rgba_f32());
            })*
        };
    }

    macro_rules! fill_triangle {
        ($p1:expr, $p2:expr, $p3:expr) =>{
            add_points!($p1, $p2, $p3);
        }
    }

    if team == Team::Red {
        let (p1, p2, p3, p4) = if activated {
            (
            (w / -2., rect_h),  (w * 0.15, rect_h),
            (w / -2., -rect_h), (0., -rect_h),
            )
        } else {
            (
            (w / -2., rect_h),  (0., rect_h),
            (w / -2., -rect_h), (w * -0.18, -rect_h),
            )
        };
        fill_triangle!(p1, p2, p3);
        fill_triangle!(p2, p3, p4);
    } else if team == Team::Black {
        let (p1, p2, p3, p4) = if activated {
            (
            (w * 0.04, rect_h), (w / 2., rect_h),
            (w * -0.15, -rect_h), (w / 2., -rect_h),
            )
        } else {
            (
            (w * 0.18, rect_h), (w / 2., rect_h),
            (w * 0.04, -rect_h), (w / 2., -rect_h),
            )
        };
        fill_triangle!(p1, p2, p3);
        fill_triangle!(p2, p3, p4);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    //mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertice_colors);
    mesh
}

fn get_team_suggestion_color(team: Team, activated: bool)-> Color {
    match team {
        Team::Red => {
            if activated {
                Color::rgba(0.95, 0.26, 0.27, 0.95)
            } else {
                Color::rgba(0.81, 0.50, 0.51, 0.95)
            }
        },
        Team::Black => {
            if activated {
                Color::rgba(0.67, 0.67, 0.67, 0.95)
            } else {
                Color::rgba(0.82, 0.82, 0.82, 0.95)
            }
        },
    }
}

/// 确定棋子可以走哪几格
/// (实在是想不到用什么名字了(· д ·))
fn get_where_can_go(target: Entity, world: &World, chess_query: &QueryState<(Entity, &Chess), With<Ingame>>)-> Vec<(i32, i32)> {
    let mut reachable_points = get_reachable_points(target, world, chess_query);

    let chess = chess_query.get_manual(world, target).unwrap().1;

    if chess.role == Role::King {
        reachable_points.retain(|&p| {
            for (entity, iter_chess) in chess_query.iter_manual(world) {
                if chess.position != iter_chess.position
                    && get_reachable_points(entity, world, chess_query)
                        .contains(&p)
                    && chess.team != iter_chess.team
                {
                    return false;
                }
            }
            true
        });
    } else if chess.role == Role::Cannon {
        fn is_invalid(pos: &(i32, i32))-> bool {
            pos.0 < 0 || pos.0 > 8 || pos.1 < 0 || pos.1 > 9
        }

        let get_chess_at = |position: (i32, i32)|-> Option<&Chess> {
            if is_invalid(&position) {
                None
            } else {
                if let Some((_, chess)) = chess_query.iter_manual(world).find(|i| i.1.position == position) {
                    Some(chess)
                } else {
                    None
                }
            }
        };

        let points = &mut reachable_points;
        let p = &chess.position;
        points.clear();
        macro_rules! cannon_search_path {
            ($loop_name:ident; $update:expr) => {
                let mut $loop_name = 0;
                for _ in 1.. {
                    $loop_name += 1;
                    let current_point = $update;
                    if let Some(_) = get_chess_at(current_point) {
                        break;
                    } else if is_invalid(&current_point) {
                        break;
                    } else {
                        points.push(current_point);
                    }
                }
                for _ in 0.. {
                    $loop_name += 1;
                    let current_point = $update;
                    if let Some(chess_in_path) = get_chess_at(current_point) {
                        if chess_in_path.team != chess.team {
                            points.push(current_point);
                        }
                        break;
                    } else if is_invalid(&current_point) {
                        break;
                    }
                }
            };
        }

        cannon_search_path!(i; (p.0 + i, p.1));
        cannon_search_path!(i; (p.0 - i, p.1));
        cannon_search_path!(i; (p.0, p.1 + i));
        cannon_search_path!(i; (p.0, p.1 - i));
    }

    // 排除友方棋子
    reachable_points.retain(|&p| {
        if let Some((_, chess_at_point)) = chess_query.iter_manual(world).find(|i| i.1.position == p) {
            if chess_at_point.team == chess.team {
                return false;
            }
        }
        true
    });

    reachable_points
}

/// 确定棋子可以够到的格子
fn get_reachable_points(target: Entity, world: &World, chess_query: &QueryState<(Entity, &Chess), With<Ingame>>)-> Vec<(i32, i32)> {
    let chess = chess_query.get_manual(world, target).unwrap().1;
    let mut points = Vec::new();
    let get_chess_at = |position: (i32, i32)|-> Option<&Chess> {
        if is_invalid(&position) {
            None
        } else {
            if let Some((_, chess)) = chess_query.iter_manual(world).find(|i| i.1.position == position) {
                Some(chess)
            } else {
                None
            }
        }
    };

    fn is_invalid(pos: &(i32, i32))-> bool {
        pos.0 < 0 || pos.0 > 8 || pos.1 < 0 || pos.1 > 9
    }

    let p = &chess.position;
    match chess.role {
        Role::King => {
            points.push((p.0 - 1, p.1));
            points.push((p.0 + 1, p.1));
            points.push((p.0, p.1 - 1));
            points.push((p.0, p.1 + 1));
            points.retain(|&p| !(
                p.0 < 3 || p.0 > 5 ||
                (2 < p.1 && p.1 < 7)
            ));
        },

        Role::Guard => {
            points.push((p.0 - 1, p.1 - 1));
            points.push((p.0 - 1, p.1 + 1));
            points.push((p.0 + 1, p.1 - 1));
            points.push((p.0 + 1, p.1 + 1));
            points.retain(|&p| !(
                p.0 < 3 || p.0 > 5 ||
                (2 < p.1 && p.1 < 7)
            ));
        },

        Role::Bishop(flag) => {
            if get_chess_at((p.0 - 1, p.1 - 1)).is_none() {
                points.push((p.0 - 2, p.1 - 2));
            }
            if get_chess_at((p.0 - 1, p.1 + 1)).is_none() {
                points.push((p.0 - 2, p.1 + 2));
            }
            if get_chess_at((p.0 + 1, p.1 - 1)).is_none() {
                points.push((p.0 + 2, p.1 - 2));
            }
            if get_chess_at((p.0 + 1, p.1 + 1)).is_none() {
                points.push((p.0 + 2, p.1 + 2));
            }
            if !flag {
                points.retain(|&p| p.1 <= 4);
            } else {
                points.retain(|&p| p.1 >= 5);
            }
        },

        Role::Horse => {
            if get_chess_at((p.0, p.1 - 1)).is_none() {
                points.push((p.0 - 1, p.1 - 2));
                points.push((p.0 + 1, p.1 - 2));
            }
            if get_chess_at((p.0, p.1 + 1)).is_none() {
                points.push((p.0 - 1, p.1 + 2));
                points.push((p.0 + 1, p.1 + 2));
            }
            if get_chess_at((p.0 - 1, p.1)).is_none() {
                points.push((p.0 - 2, p.1 - 1));
                points.push((p.0 - 2, p.1 + 1));
            }
            if get_chess_at((p.0 + 1, p.1)).is_none() {
                points.push((p.0 + 2, p.1 - 1));
                points.push((p.0 + 2, p.1 + 1));
            }
        },

        Role::Chariot => {
            macro_rules! chariot_search_path {
                ($loop_name:ident; $update:expr) => {
                    for $loop_name in 1.. {
                        let current_point = $update;
                        if let Some(chess_in_path) = get_chess_at(current_point) {
                            if chess_in_path.team != chess.team {
                                points.push(current_point);
                                break;
                            } else {
                                break;
                            }
                        } else if is_invalid(&current_point) {
                            break;
                        } else {
                            points.push(current_point);
                        }
                    }
                };
            }

            chariot_search_path!(i; (p.0 - i, p.1));
            chariot_search_path!(i; (p.0 + i, p.1));
            chariot_search_path!(i; (p.0, p.1 - i));
            chariot_search_path!(i; (p.0, p.1 + i));
        },

        Role::Cannon => {
            macro_rules! cannon_search_path {
                ($loop_name:ident; $update:expr) => {
                    let mut $loop_name = 0;
                    for _ in 1.. {
                        $loop_name += 1;
                        let current_point = $update;
                        if let Some(_) = get_chess_at(current_point) {
                            break;
                        } else if is_invalid(&current_point) {
                            break;
                        } else {
                            points.push(current_point);
                        }
                    }
                    for _ in 0.. {
                        $loop_name += 1;
                        let current_point = $update;
                        if let Some(chess_in_path) = get_chess_at(current_point) {
                            if chess_in_path.team != chess.team {
                                points.push(current_point);
                            }
                            break;
                        } else if is_invalid(&current_point) {
                            break;
                        } else {
                            points.push(current_point);
                        }
                    }
                };
            }

            cannon_search_path!(i; (p.0 + i, p.1));
            cannon_search_path!(i; (p.0 - i, p.1));
            cannon_search_path!(i; (p.0, p.1 + i));
            cannon_search_path!(i; (p.0, p.1 - i));
        },

        Role::Pawn(flag) => {
            if !flag {
                points.push((p.0, p.1 + 1));
                if p.1 > 4 {
                    points.push((p.0 - 1, p.1));
                    points.push((p.0 + 1, p.1));
                }
            } else {
                points.push((p.0, p.1 - 1));
                if p.1 < 5 {
                    points.push((p.0 - 1, p.1));
                    points.push((p.0 + 1, p.1));
                }
            }
        },
    }

    points.retain(|&p| !is_invalid(&p));

    points
}





fn game_setup_system(
    mut commands: Commands,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
) {
    let window_size = WindowSize(400., 700.);

    commands.spawn(Camera2dBundle {
        camera_2d: Camera2d {
            clear_color: bevy::core_pipeline::clear_color::ClearColorConfig::Custom(Color::rgb(0.9, 0.9, 0.9)),
        },
        ..Default::default()
    });

    {
        let mut window = window_query.single_mut();
        window.position = WindowPosition::Centered(MonitorSelection::Current);
        window.resolution = WindowResolution::new(window_size.0, window_size.1);
        window.title = "Chinese Chess Game".to_string();
    }

    commands.insert_resource(window_size.clone());

    let padding = Val::Percent(6.);
    commands.spawn((
        Ingame,
        Chessboard,
        NodeBundle {
            style: Style {
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Stretch,
                size: Size::new(Val::Px(window_size.0), Val::Px(window_size.0 * 1.111)),
                flex_direction: FlexDirection::Column,
                aspect_ratio: Some(1.0),
                padding: UiRect {
                    left: padding,
                    right: padding,
                    top: padding,
                    bottom: padding,
                },
                gap: Size::new(Val::Px(10.), Val::Px(10.)),
                ..Default::default()
            },
            ..Default::default()
        }
    )).with_children(|parent| {
        for y in 0..10 {
            parent.spawn((
                Ingame,
                NodeBundle {
                    style: Style {
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Stretch,
                        flex_direction: FlexDirection::Row,
                        size: Size::new(Val::Auto, Val::Percent(100.)),
                        gap: Size::new(Val::Px(10.), Val::Px(10.)),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )).with_children(|parent| {
                for x in 0..9 {
                    parent.spawn((
                        Ingame,
                        ChessButton { x, y },
                        ButtonBundle {
                            style: Style {
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Stretch,
                                size: Size::new(Val::Percent(100.), Val::Auto),
                                ..Default::default()
                            },
                            background_color: BackgroundColor::from(Color::rgba(1., 1., 1., 0.)),
                            ..Default::default()
                        }
                    ));
                }
            });
        }
    });

    commands.spawn((
        Ingame,
        Chessboard,
        MaterialMesh2dBundle {
            mesh: Mesh2dHandle(meshes.add(create_chessboard_mesh(window_size.clone()))),
            material: color_materials.add(ColorMaterial::from(Color::rgb(0., 0., 0.))),
            visibility: Visibility::Visible,
            ..Default::default()
        }
    ));

    use once_cell::sync::OnceCell;
    static CHESSES: OnceCell<Vec<(Role, (i32, i32))>> = OnceCell::new();
    if CHESSES.get().is_none() {
        CHESSES.set(vec![
            (Role::Chariot,       (0, 0)),
            (Role::Horse,         (1, 0)),
            (Role::Bishop(false), (2, 0)),
            (Role::Guard,         (3, 0)),
            (Role::King,          (4, 0)),
            (Role::Guard,         (5, 0)),
            (Role::Bishop(false), (6, 0)),
            (Role::Horse,         (7, 0)),
            (Role::Chariot,       (8, 0)),
            (Role::Cannon,        (1, 2)),
            (Role::Cannon,        (7, 2)),
            (Role::Pawn(false),   (0, 3)),
            (Role::Pawn(false),   (2, 3)),
            (Role::Pawn(false),   (4, 3)),
            (Role::Pawn(false),   (6, 3)),
            (Role::Pawn(false),   (8, 3)),
        ]).unwrap();
    }
    // 生成棋子
    for info in CHESSES.wait().iter() {
        commands.spawn((
            Ingame,
            Chess {
                team: Team::Black,
                role: info.0,
                position: info.1,
                redraw_stage: 0,
            },
        ));

        commands.spawn((
            Ingame,
            Chess {
                team: Team::Red,
                role: match info.0 {
                    Role::Pawn(flag) => Role::Pawn(!flag),
                    Role::Bishop(flag) => Role::Bishop(!flag),
                    _ => info.0,
                },
                position: (info.1.0, 9 - info.1.1),
                redraw_stage: 0,
            }
        ));
    }

    commands.init_resource::<CurrentTeam>();
    commands.init_resource::<History>();

    let t = Transform::from_xyz(0., window_size.1 / 2. - window_size.0 * 1.250, 0.7);
    commands.spawn((
        Ingame,
        TeamSuggestion(Team::Red),
        MaterialMesh2dBundle {
            mesh: Mesh2dHandle(meshes.add(create_team_suggestion_mesh(window_size, Team::Red, true))),
            transform: t,
            material: color_materials.add(ColorMaterial::from(get_team_suggestion_color(Team::Red, true))),
            ..Default::default()
        },
    ));
    commands.spawn((
        Ingame,
        TeamSuggestion(Team::Black),
        MaterialMesh2dBundle {
            mesh: Mesh2dHandle(meshes.add(create_team_suggestion_mesh(window_size, Team::Black, false))),
            transform: t,
            material: color_materials.add(ColorMaterial::from(get_team_suggestion_color(Team::Black, false))),
            ..Default::default()
        },
    ));
}

fn game_system(
    mut commands: Commands,
    mut history: ResMut<History>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut current_team: ResMut<CurrentTeam>,
    children_query: Query<&Children>,
    mut button_set: ParamSet<(
        Query<(&ChessButton, &Interaction), (Changed<Interaction>, With<Button>, With<Ingame>)>,
        Query<&Interaction, (Changed<Interaction>, With<Button>, With<Ingame>, With<UndoButton>)>,
    )>,
    preview_query: Query<&PreviewPoint>,
    preview_entity_query: Query<Entity, With<PreviewPoint>>,
    mut set: ParamSet<(
        Query<(Entity, &Selected, &mut Chess, &mut Transform, &mut Handle<ColorMaterial>), With<Ingame>>,
        Query<(Entity, &mut Chess, &mut Handle<ColorMaterial>), With<Ingame>>,
    )>,
) {
    for (button, interaction) in button_set.p0().iter() {
        let new_material = if *interaction == Interaction::Hovered {
            color_materials.add(ColorMaterial::from(CHESS_HOVERED_COLOR))
        } else {
            color_materials.add(ColorMaterial::from(CHESS_NORMAL_COLOR))
        };
        for mut i in set.p1().iter_mut() {
            if i.1.position.0 == button.x && i.1.position.1 == button.y {
                *i.2 = new_material.clone();
            }
        }

        if *interaction == Interaction::Clicked {
            // 没有已经选中的棋子
            if set.p0().is_empty() {
                for mut i in set.p1().iter_mut() {
                    if i.1.position.0 == button.x && i.1.position.1 == button.y && i.1.redraw_stage == 0 {
                        if i.1.team == current_team.0 {
                            // 选中
                            commands.entity(i.0).insert(Selected);
                            i.1.redraw_stage = 1;

                            commands.add(move |world: &mut World| {
                                let chess_query = world.query_filtered::<(Entity, &Chess), With<Ingame>>();
                                // 生成预览点
                                let preview_points = get_where_can_go(i.0, &world, &chess_query);
                                preview_points.iter().for_each(|&p| {
                                    world.spawn((
                                        Ingame,
                                        PreviewPoint(p.0, p.1)
                                    ));
                                });
                            });
                        }
                    }
                }
            } else {
            // 有已经选中的棋子
                if set.p0().single().2.position == (button.x, button.y) {
                    // 清除选中，因为点击了同一个棋子
                    set.p0().iter_mut().for_each(|mut i| {
                        if i.2.redraw_stage == 0 {
                            commands.entity(i.0).remove::<Selected>();
                            i.2.redraw_stage = 1;
                        }
                    });
                    // 清除预览点
                    preview_entity_query.iter().for_each(|e| commands.entity(e).despawn());
                } else if preview_query.iter().find(|&p| p.0 == button.x && p.1 == button.y).is_some() {
                    current_team.0 = current_team.0.opposite();

                    let mut target_chess = None::<Chess>;
                    // 清除目标格子的棋子和字
                    set.p1().iter().for_each(|i| {
                        if i.1.position == (button.x, button.y) {
                            target_chess = Some(i.1.clone());
                            commands.entity(i.0).despawn();
                            children_query.iter_descendants(i.0)
                                .for_each(|child| commands.entity(child).despawn());
                        }
                    });
                    let mut p0 = set.p0();
                    let query_result = p0.single_mut();
                    let mut selected_chess = query_result.2;
                    // 添加历史记录
                    history.push(HistoryRecord {
                        from_pos: selected_chess.position,
                        to_pos: (button.x, button.y),
                        target_chess: target_chess,
                    });
                    // 将当前棋子移动到目标位置，清除选中
                    selected_chess.position = (button.x, button.y);
                    selected_chess.redraw_stage = 1;
                    commands.entity(query_result.0).remove::<Selected>();

                    // 清除预览点
                    preview_entity_query.iter().for_each(|e| commands.entity(e).despawn());
                }
            }
        }
    }

    for interaction in button_set.p1().iter() {
    }
}



fn window_size_update_system(
    mut window_size: ResMut<WindowSize>,
    window_query: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut window_resize_events: EventReader<WindowResized>
) {
    let (entity, window) = window_query.single();
    for resize_event in window_resize_events.iter() {
        if resize_event.window == entity {
            window_size.0 = window.resolution.width();
            window_size.1 = window.resolution.height();
            break;
        }
    }
}

fn chessboard_system(
    mut commands: Commands,
    window_size: Res<WindowSize>,
    asset_server: Res<AssetServer>,
    children_query: Query<&Children>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut chessboard_node_query: Query<&mut Style, (With<Chessboard>, With<Node>)>,
    mut set: ParamSet<(
        Query<&mut Mesh2dHandle, (With<Chessboard>, Without<Node>)>,
        Query<(Entity, &mut Chess, Option<&mut Transform>, Option<&mut Mesh2dHandle>, Option<&Selected>, Option<&mut TransformAnimation>), With<Ingame>>,
        Query<(Entity, &PreviewPoint, Option<&mut Transform>, Option<&mut Mesh2dHandle>)>,
    )>,
) {
    if window_size.is_changed() {
        let mut size = &mut chessboard_node_query.single_mut().size;
        size.width = Val::Px(window_size.0);
        size.height = Val::Px(window_size.0);

        let mut q = set.p0();
        let mesh = &mut q.single_mut().0;
        mesh.clone_from(&meshes.add(create_chessboard_mesh(*window_size)));
    }



    let mesh = Mesh2dHandle(meshes.add(create_chess_mesh(*window_size)));
    let chess_material = color_materials.add(ColorMaterial::from(CHESS_NORMAL_COLOR));
    let width = window_size.compute_width();
    let height = window_size.compute_height();
    let padding = window_size.compute_padding();
    let button_size = window_size.compute_button_size();
    for (entity, mut chess, transform, old_mesh, selected, anim) in set.p1().iter_mut() {
        if let Some(mut transform) = transform {
            if chess.redraw_stage == 1 {
                chess.redraw_stage = 2;
            } else if window_size.is_changed() || chess.redraw_stage == 2 {
                let mut anim = anim.unwrap();
                // 更新棋子的位置和大小
                let chess_pos = screen_to_bevy(*window_size, (
                    padding + width * chess.position.0 as f32 / 8.,
                    padding + height * chess.position.1 as f32 / 9.
                ));
                *old_mesh.unwrap() = mesh.clone();
                anim.begin_state = *transform;
                anim.end_state = *transform;
                let t = &mut anim.end_state.translation;
                t.x = chess_pos.0;
                t.y = chess_pos.1;
                anim.end_state.scale = if selected.is_some() {
                    Vec3::new(1.3, 1.3, 1.)
                } else {
                    Vec3::new(1., 1., 1.)
                };
                anim.activate();
                anim.init_time(Duration::from_millis(825));

                // 更新字体大小
                // 位置不用更新，因为字的位置始终在按钮中心
                for child in children_query.iter_descendants(entity) {
                    commands.add(move |world: &mut World| {
                        let mut child = world.entity_mut(child);
                        if let Some(mut text) = child.get_mut::<Text>() {
                            for mut section in text.sections.iter_mut() {
                                section.style.font_size = button_size * 0.86;
                            }
                        }
                        let new_scale = if child.contains::<Selected>() {
                            Vec3::new(1.3, 1.3, 1.)
                        } else {
                            Vec3::new(1., 1., 1.)
                        };
                        if let Some(mut transform) = child.get_mut::<Transform>() {
                            transform.scale = new_scale;
                        }
                    });
                }

                chess.redraw_stage = 0;
            }
        } else {
            fn func(mut progress: f32)-> f32 {
                if !(0. <= progress && progress <= 1.) {
                    progress = if progress > 1. {
                        1.
                    } else {
                        0.1
                    }
                }
                1. - (2f32).powf(progress * -10.)
            }
            // 初始化棋子的贴图
            let chess_pos = screen_to_bevy(*window_size, (
                padding + width * chess.position.0 as f32 / 8.,
                padding + height * chess.position.1 as f32 / 9.,
            ));
            commands.entity(entity)
            .insert((
                MaterialMesh2dBundle {
                    mesh: mesh.clone(),
                    material: chess_material.clone(),
                    transform: Transform::from_xyz(chess_pos.0, chess_pos.1, 0.1),
                    ..Default::default()
                },
                TransformAnimation {
                    timing_function: func,
                    ..Default::default()
                }
            ))
            .with_children(|parent| {
                parent.spawn(Text2dBundle {
                    text: Text {
                        alignment: TextAlignment::Center,
                        linebreak_behaviour: bevy::text::BreakLineOn::AnyCharacter,
                        sections: vec![
                            TextSection {
                                value: chess.to_owned_string(),
                                style: TextStyle {
                                    color: chess.get_color(),
                                    font_size: button_size * 0.86,
                                    font: asset_server.load("LXGWWenKai-subset.ttf"),
                                },
                            }
                        ],
                    },
                    text_anchor: bevy::sprite::Anchor::Center,
                    transform: Transform::from_xyz(0., 0., 0.2),
                    ..Default::default()
                });
            });
        }
    }

    let mesh = Mesh2dHandle(meshes.add(create_preview_point_mesh(*window_size)));
    let point_material = color_materials.add(ColorMaterial::from(PREVIEW_POINT_COLOR));
    for (entity, point, transform, old_mesh) in set.p2().iter_mut() {
        let point_pos = screen_to_bevy(*window_size, (
            padding + width * point.0 as f32 / 8.,
            padding + height * point.1 as f32 / 9.,
        ));
        if let Some(mut transform) = transform {
            if window_size.is_changed() {
                let mut t = &mut transform.translation;
                t.x = point_pos.0;
                t.y = point_pos.1;

                *old_mesh.unwrap() = mesh.clone();
            }
        } else {
            commands.entity(entity).insert(MaterialMesh2dBundle {
                mesh: mesh.clone(),
                material: point_material.clone(),
                transform: Transform::from_xyz(point_pos.0, point_pos.1, 0.5),
                ..Default::default()
            });
        }
    }
}

fn team_suggestion_system(
    current_team: Res<CurrentTeam>,
    window_size: Res<WindowSize>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut team_suggestion_query: Query<(&TeamSuggestion, &mut Transform, &mut Mesh2dHandle, &mut Handle<ColorMaterial>)>,
) {
    for (team_suggestion, mut transform, mut mesh, mut color) in team_suggestion_query.iter_mut() {
        if current_team.is_changed() || window_size.is_changed() {
            let team = team_suggestion.0;
            let activated = current_team.0 == team_suggestion.0;

            *mesh = Mesh2dHandle(meshes.add(create_team_suggestion_mesh(*window_size, team_suggestion.0, activated)));
            let mut t = &mut transform.translation;
            t.x = 0.;
            t.y = window_size.1 / 2. - window_size.0 * 1.250;

            *color = color_materials.add(ColorMaterial::from(get_team_suggestion_color(team, activated)));
        }
    }
}

fn transform_animation_system(
    window_size: Res<WindowSize>,
    mut old_ws: Local<WindowSize>,
    mut animation_query: Query<(&mut TransformAnimation, &mut Transform)>,
) {
    if old_ws.0 == 0. || old_ws.1 == 0. {
        *old_ws = *window_size;
    }

    for (mut anim, mut transform) in animation_query.iter_mut() {
        if window_size.is_changed() {
            let x_ratio = window_size.0 / old_ws.0;
            let y_ratio = window_size.1 / old_ws.1;
            let mut t = &mut anim.begin_state.translation;
            t.x *= x_ratio;
            t.y *= y_ratio;
            let mut t = &mut anim.end_state.translation;
            t.x *= x_ratio;
            t.y *= y_ratio;
            let mut t = &mut transform.translation;
            t.x *= x_ratio;
            t.y *= y_ratio;
        }
        if anim.is_done() {
            anim.unactivate();
        } else if anim.activated {
            *transform = anim.transform();
        }
    }

    if window_size.is_changed() {
        *old_ws = *window_size;
    }
}
