use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{ Application, ApplicationWindow };
use glib::{ Priority, MainContext, clone };
use rand::Rng;
use once_cell::sync::OnceCell;
use std::cell::RefCell;
use std::sync::{ Arc, Mutex };
use std::io::{ Write, Read };
use std::net::*;
use std::rc::Rc;
use std::thread;
use std::time::{ Instant, SystemTime, Duration, UNIX_EPOCH };

static PADDING_RATIO: f64 = 0.1;

static STATUS_BAR_INITIAL_TEXT: &'static str = "这里是状态栏\\(￣3￣)/";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DiscoverState {
    Pause,
    Stop,
    Continue,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ConnectStage {
    No,
    Waiting {
        role: Role,
        opponent_name: Option<String>,
        prepared: bool,
    },
    Gaming,
    Gameovered,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Owner,

    Invalid,

    /// 对应的数值为0
    Player,

    /// 对应的数值为1
    Visitor,
}

impl Role {
    pub fn to_u8(&self)-> u8 {
        match *self {
            Self::Player => 0,
            Self::Visitor => 1,
            _ => panic!(),
        }
    }
}

impl From<u8> for Role {
    fn from(val: u8)-> Self {
        match val {
            0 => Self::Player,
            1 => Self::Visitor,
            _ => Self::Invalid,
        }
    }
}

/// 每个网络包都以一个u32开头，这个u32就是MessageType
/// 接下来就是数据包的具体内容了
#[derive(Debug)]
#[non_exhaustive]
enum NetworkEvent {
    /// MessageType: 114514
    /// ErrorLength: u8
    /// ErrorString: ErrorLength长的u8
    Error(String),

    /// 请求加入一个房间
    /// MessageType: 0
    /// Role: u8
    /// NameLength: u8
    /// NameString: NameLength个字节长的字符串
    EnterRoom {
        name: String,
        role: Role,
    },

    /// 准许以role描述的身份进入房间
    /// MessageType: 1
    /// Role: u8
    /// PeerNameLength: u8
    /// PeerNameString: PeerNameLength个字节长的字符串
    EnterPermitted {
        role: Role,
        name: String
    },

    /// 表示房间已满
    /// MessageType: 2
    RoomIsFull,

    /// 心跳包
    /// MessageType: 3
    /// SendTime: 发送时间，u64表示的时间戳(毫秒级别)
    Ping {
        send_time: SystemTime,
    },

    /// 心跳包的回应
    /// MessageType: 4
    Pong {
        send_time: SystemTime,
    },

    /// 房间已解散
    /// MessageType: 5
    RoomDisbanded,

    /// 离开房间
    /// MessageType: 6
    LeaveRoom,

    /// 设置准备状态
    /// MessageType: 7
    /// value: u8，0表示false，非0表示true
    SetPrepared(bool),

    /// 五子棋，启动！
    /// MessageType: 8
    StartGame,

    /// 下棋
    /// MessageType: 9
    /// x: u8  X坐标
    /// y: u8  Y坐标
    PutChess {
        x: u8,
        y: u8,
    },

    /// 聊天信息
    /// MessageType: 10
    /// ContentLength: u16  消息长度
    /// ContentString: ContentLength长度的字符串(UTF8-ENCODED)
    ChatMessage(String),

    /// 悔棋请求
    /// MessageType: 11
    UndoRequest,

    /// 下棋成功的回应
    /// MessageType: 12
    PutChessSucceed,

    /// 对于悔棋的回应
    /// MessageType: 13
    /// Allowed: u8 -> bool    是否同意悔棋
    UndoReply(bool),

    /// 逃跑
    /// MessageType: 14
    Escape,
}

impl NetworkEvent {
    pub fn from_buffer(buf: &[u8])-> Option<(Self, usize)> {
        let bytes_available = buf.len();
        let mut bytes_read = 0usize;
 
        macro_rules! _assert {
            () => {
                if bytes_available < bytes_read {
                    return None;
                }
            };
        }

        macro_rules! read_to_slice {
            ($length:expr) => {{
                bytes_read += $length;
                _assert!();
                &buf[(bytes_read - ($length))..bytes_read]
            }};
        }

        macro_rules! read_u8 {
            () => {{
                bytes_read += 1;
                _assert!();
                u8::from_be(buf[bytes_read - 1])
            }};
        }

        macro_rules! read_role {
            () => {{
                let role_u8 = read_u8!();
                let role = Role::from(role_u8);
                if role == Role::Invalid {
                    return Some((NetworkEvent::Error(format!("Invalid role ID {role_u8}")), 0));
                }
                role
            }};
        }

        macro_rules! read_string_u8len {
            () => {{
                let string_length = read_u8!() as usize;
                String::from_utf8_lossy(read_to_slice!(string_length)).to_string()
            }};
        }

        macro_rules! read_string_u16len {
            () => {{
                let string_length = u16::from_be_bytes(read_to_slice!(2).try_into().unwrap()) as usize;
                String::from_utf8_lossy(read_to_slice!(string_length)).to_string()
            }};
        }

        macro_rules! read_time {
            () => {{
                let ms = u64::from_be_bytes(read_to_slice!(8).try_into().unwrap());
                UNIX_EPOCH + Duration::from_millis(ms)
            }};
        }

        let msgid = u32::from_be_bytes(read_to_slice!(4).try_into().unwrap());
        let event = match msgid {
            0 => {
                let role = read_role!();
                let name = read_string_u8len!();

                NetworkEvent::EnterRoom { role, name }
            },

            1 => {
                let role = read_role!();
                let name = read_string_u8len!();

                NetworkEvent::EnterPermitted { role, name }
            },

            2 => {
                NetworkEvent::RoomIsFull
            },

            3 => {
                NetworkEvent::Ping { send_time: read_time!() }
            },

            4 => {
                NetworkEvent::Pong { send_time: read_time!() }
            },

            5 => {
                NetworkEvent::RoomDisbanded
            },

            6 => {
                NetworkEvent::LeaveRoom
            },

            7 => {
                let v = match read_u8!() {
                    0 => false,
                    _ => true,
                };

                NetworkEvent::SetPrepared(v)
            },

            8 => {
                NetworkEvent::StartGame
            },

            9 => {
                let x = read_u8!();
                let y = read_u8!();

                NetworkEvent::PutChess { x, y }
            },

            10 => {
                NetworkEvent::ChatMessage(read_string_u16len!())
            },

            11 => {
                NetworkEvent::UndoRequest
            },

            12 => {
                NetworkEvent::PutChessSucceed
            },

            13 => {
                let v = match read_u8!() {
                    0 => false,
                    _ => true,
                };

                NetworkEvent::UndoReply(v)
            },

            14 => {
                NetworkEvent::Escape
            },

            114514 => {
                NetworkEvent::Error(read_string_u8len!())
            },

            _ => {
                NetworkEvent::Error(format!("Unrecognizable message type {msgid}"))
            },
        };

        Some((event, bytes_read))
    }

    pub fn to_u8_vec(&self)-> Vec<u8> {
        let mut buf = Vec::new();

        macro_rules! push_int {
            ($val:expr) => {
                ($val).to_be_bytes()
                    .into_iter()
                    .for_each(|i| buf.push(i));
            };
        }

        macro_rules! push_string_u8len {
            ($val:expr) => {
                let bytes_form = ($val).as_bytes();
                push_int!(bytes_form.len() as u8);
                bytes_form.iter().for_each(|i| buf.push(*i));
            };
        }

        macro_rules! push_string_u16len {
            ($val:expr) => {
                let bytes_form = ($val).as_bytes();
                push_int!(bytes_form.len() as u16);
                bytes_form.iter().for_each(|i| buf.push(*i));
            };
        }

        match self {
            Self::Error(ref msg) => {
                push_int!(114_514u32);
                push_string_u8len!(msg);
            },

            Self::EnterRoom { ref role, ref name } => {
                push_int!(0u32);
                buf.push(role.to_u8().to_be());
                push_string_u8len!(name);
            },

            Self::EnterPermitted { ref role, ref name } => {
                push_int!(1u32);
                buf.push(role.to_u8().to_be());
                push_string_u8len!(name);
            },

            &Self::RoomIsFull => {
                push_int!(2u32);
            },

            Self::Ping { ref send_time } => {
                push_int!(3u32);
                push_int!(send_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64);
            },

            Self::Pong { ref send_time } => {
                push_int!(4u32);
                push_int!(send_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64);
            },

            &Self::RoomDisbanded => {
                push_int!(5u32);
            },

            &Self::LeaveRoom => {
                push_int!(6u32);
            },

            Self::SetPrepared(ref v) => {
                push_int!(7u32);
                match *v {
                    true => buf.push(1u8.to_be()),
                    false => buf.push(0u8.to_be()),
                }
            },

            &Self::StartGame => {
                push_int!(8u32);
            },

            Self::PutChess { ref x, ref y } => {
                push_int!(9u32);
                push_int!(*x);
                push_int!(*y);
            },

            Self::ChatMessage(ref msg) => {
                push_int!(10u32);
                push_string_u16len!(msg);
            },

            &Self::UndoRequest => {
                push_int!(11u32);
            },

            &Self::PutChessSucceed => {
                push_int!(12u32);
            },

            Self::UndoReply(ref v) => {
                push_int!(13u32);
                match *v {
                    true => buf.push(1u8.to_be()),
                    false => buf.push(0u8.to_be()),
                }
            },

            &Self::Escape => {
                push_int!(14u32);
            },
        }

        buf
    }
}

fn main()-> glib::ExitCode {
    let app = Application::builder()
        .application_id(&format!("org.xuanyeovo.gomoku_gtk_rs{}", rand::thread_rng().gen_range(0..66666)))
        .build();

    app.connect_activate(build_ui);

    app.run()
}

fn build_ui(app: &Application) {
    fn area_to_grid(area_size: f64, mut pos: (f64, f64))-> Option<(isize, isize)> {
        let padding = area_size * PADDING_RATIO;
        let content_size = area_size - padding * 2.0;
        let cell_size = content_size / 14.0;
        let react_padding = padding - cell_size / 2.0;

        if pos.0 <= react_padding || pos.1 <= react_padding || pos.0 >= area_size - react_padding || pos.1 >= area_size - react_padding {
            None
        } else {
            pos.0 -= react_padding;
            pos.1 -= react_padding;

            let x_times = pos.0 / cell_size;
            let y_times = pos.1 / cell_size;

            let xf = x_times.fract();
            let yf = y_times.fract();
            if xf < 0.1 || 0.9 < xf || yf < 0.1 || 0.9 < yf {
                None
            } else {
                Some((x_times.floor() as isize, y_times.floor() as isize))
            }
        }
    }

    let grid = Arc::new(RefCell::new(ChessboardGrid::new()));
    let state = Arc::new(Mutex::new(State::default()));
    let pressed = Rc::new(std::cell::Cell::new( None::<(isize, isize)> ));
    let discover = Arc::new(Mutex::new(DiscoverState::Stop));
    let connect_stage = Arc::new(Mutex::new(ConnectStage::No));
    let daemon_running = Arc::new(Mutex::new(false));
    let last_pong = Arc::new(Mutex::new(Instant::now()));
    let last_ping = Arc::new(Mutex::new(Instant::now()));

    // 用于触发事件处理函数
    let (event_sender, event_receiver) = MainContext::channel(Priority::default());

    // 用于在事件处理函数里发送数据包
    let (cl_sender, cl_receiver) = std::sync::mpsc::channel();
    let cl_receiver = Arc::new(Mutex::new(cl_receiver));





    let stack = gtk::Stack::builder()
        .hexpand(true)
        .vexpand(true)
        .build();

    let win = ApplicationWindow::builder()
        .application(app)
        .default_width(480)
        .default_height(800)
        .child(&stack)
        .title("Gomoku Game( Rust + GTK4 )")
        .build();


    // 游戏页面
    let game_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .hexpand(true)
        .vexpand(true)
        .build();

    let status_bar = gtk::Label::builder()
        .label(STATUS_BAR_INITIAL_TEXT)
        .hexpand(true)
        .valign(gtk::Align::End)
        .build();

    let chessboard_area = gtk::DrawingArea::builder()
        .hexpand(true)
        .build();
    chessboard_area.set_draw_func(clone!(
    @strong grid => move |da, c, width, height| {
        use std::f64::consts::PI;

        if width != height {
            //da.set_content_width(width);
            da.set_content_height(width);
        }
        let w = width as f64;
        let padding = w * PADDING_RATIO;
        let content_w = w - padding * 2.0;

        // 绘制棋盘
        c.set_source_rgba(1.0, 0.88, 0.80, 1.0);
        c.rectangle(0.0, 0.0, w, w);
        c.fill().unwrap();

        let line_w = if w < 600.0 {
            1.0
        } else {
            w / 600.0
        };

        c.new_path();
        c.set_line_width(line_w);
        c.set_source_rgba(0.1, 0.1, 0.1, 1.0);
        for x in 0..15 {
            let x = padding + content_w * x as f64 / 14.0;
            c.move_to(x, padding);
            c.line_to(x, w - padding);
        }
        for y in 0..15 {
            let y = padding + content_w * y as f64 / 14.0;
            c.move_to(padding, y);
            c.line_to(w - padding, y);
        }
        c.close_path();
        c.stroke().unwrap();

        // 棋盘上的五个小点
        for pos in vec![(3., 3.), (7., 7.), (11., 3.), (3., 11.), (11., 11.)].iter() {
            let cx = padding + content_w * pos.0 / 14.;
            let cy = padding + content_w * pos.1 / 14.;

            c.new_path();
            c.arc(cx, cy, content_w / 28.0 * 0.15, 0., PI * 2.);
            c.fill().unwrap();
        }

        // 绘制棋子
        let grid_ref = grid.borrow();

        for x in 0..15 {
            for y in 0..15 {
                let current_cell = grid_ref.at(x, y).unwrap();
                if current_cell.chess.is_some() {
                    let cx = padding + content_w * x as f64 / 14.0;
                    let cy = padding + content_w * y as f64 / 14.0;

                    match *current_cell.chess.as_ref().unwrap() {
                        Team::White => c.set_source_rgba(0.96, 0.96, 0.96, 1.0),
                        Team::Black => c.set_source_rgba(0.2, 0.2, 0.2, 1.0),
                    }

                    c.new_path();
                    c.arc(cx, cy, content_w / 28.0 * 0.8, 0.0, 2.0 * PI);
                    c.fill().unwrap();
                }
            }
        }
    }
    ));

    // 以固定格式更新状态栏的文本
    let upsb = clone!(
    @weak status_bar => move |team: Team| {
        status_bar.set_label(&format!("现在轮到 {} 了哒", team.as_str()));
    }
    );

    // 响应点击，根据Mode执行不同操作
    // 如果state.frozen为true则不执行任何操作
    let update_status_bar = upsb.clone();
    let do_click = clone!(
    @strong grid,
    @strong state,
    @strong cl_sender,
    @weak chessboard_area,
    @weak win,
    @weak status_bar,
    => move |(x, y)| {
        let mut state_ref = state.lock().unwrap();
        let mut grid_ref = grid.borrow_mut();

        if state_ref.frozen {
            return;
        }

        if grid_ref.at(x, y).unwrap().chess.is_some() {
            return;
        }

        if state_ref.mode.is_single_player() {
            grid_ref.at_mut(x, y).unwrap().chess = Some(state_ref.current_team);
            state_ref.current_team.set_opposite();
            state_ref.history.push((x, y));

            update_status_bar(state_ref.current_team);

            chessboard_area.queue_draw();

            // 弹出胜利提示框
            if let Some(team_win) = grid_ref.check_win() {
                state_ref.frozen = true;

                let team_str = team_win.as_str();
                let adj = get_a_good_adj();

                status_bar.set_label(&format!("{team_str} {adj}"));

                let msgbox = gtk::MessageDialog::builder()
                    .text(format!("{team_str} 赢了"))
                    .buttons(gtk::ButtonsType::Ok)
                    .message_type(gtk::MessageType::Info)
                    .transient_for(&win)
                    .modal(true)
                    .build();
                msgbox.present();
            }
        } else if state_ref.mode.is_multiple_player() {
            if let Mode::MultiplePlayer { my_team, .. } = &state_ref.mode {
                if *my_team == state_ref.current_team {
                    cl_sender.send(NetworkEvent::PutChess {x: x as u8, y: y as u8}).unwrap();

                    grid_ref.at_mut(x, y).unwrap().chess = Some(*my_team);

                    state_ref.history.push((x, y));

                    state_ref.current_team.set_opposite();

                    update_status_bar(state_ref.current_team);

                    chessboard_area.queue_draw();
                }
            } else {
                unreachable!();
            }
        }
    }
    );

    let click_reactor = gtk::GestureClick::new();

    click_reactor.connect_pressed(clone!(
    @strong pressed, @weak chessboard_area => move |_, btn, x, y| {
        if btn != 1 {
            return;
        }

        pressed.set(area_to_grid(chessboard_area.width() as f64, (x as f64, y as f64)));
    }
    ));

    click_reactor.connect_released(clone!(
    @strong pressed, @weak chessboard_area => move |_, btn, x, y| {
        if btn != 1 {
            pressed.set(None);
        }

        if area_to_grid(chessboard_area.width() as f64, (x as f64, y as f64)) == pressed.get() && pressed.get().is_some() {
            do_click(pressed.get().unwrap());
        }
        pressed.set(None);
    }
    ));

    let team_suggestion = gtk::Label::builder()
        .label("")
        .hexpand(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    // 下方工具栏(单机模式)
    let tool_bar_single_player = gtk::Box::builder()
        .hexpand(true)
        .orientation(gtk::Orientation::Horizontal)
        .build();

    // 悔棋按钮(单机模式)
    let update_status_bar = upsb.clone();
    let button_undo = gtk::Button::with_label("悔棋");
    button_undo.connect_clicked(clone!(
    @strong grid, @strong state, @weak chessboard_area => move |_| {
        let mut state_ref = state.lock().unwrap();
        let mut grid_ref = grid.borrow_mut();

        if state_ref.frozen {
            return;
        }

        if let Some(pos) = state_ref.history.pop() {
            state_ref.current_team.set_opposite();
            grid_ref.at_mut(pos.0, pos.1).unwrap().chess = None;

            update_status_bar(state_ref.current_team);

            chessboard_area.queue_draw();
        }
    }
    ));

    // 重置按钮(单机模式)
    let button_reset = gtk::Button::with_label("重置游戏");
    button_reset.connect_clicked(clone!(
    @weak status_bar,
    @strong grid,
    @strong state,
    @weak chessboard_area,
    => move |_| {
        status_bar.set_label(STATUS_BAR_INITIAL_TEXT);

        let mut grid_ref = grid.borrow_mut();
        let mut state_ref = state.lock().unwrap();

        grid_ref.clear();

        state_ref.history.clear();
        state_ref.current_team = Team::Black;
        state_ref.frozen = false;

        chessboard_area.queue_draw();
    }
    ));

    // 返回按钮(单机模式)
    let button_exit = gtk::Button::with_label("返回主界面");
    button_exit.connect_clicked(clone!(
    @weak stack => move |_| {
        stack.set_visible_child_name("title");
    }
    ));

    tool_bar_single_player.append(&button_undo);
    tool_bar_single_player.append(&button_reset);
    tool_bar_single_player.append(&button_exit);


    // 下方工具栏(联机模式)
    let tool_bar_multiple_player = gtk::Box::builder()
        .hexpand(true)
        .orientation(gtk::Orientation::Horizontal)
        .build();

    // 请求悔棋按钮(联机模式)
    let update_status_bar = upsb.clone();
    let button_undo = gtk::Button::with_label("悔棋");
    button_undo.connect_clicked(clone!(
    @strong grid, @strong state, @weak chessboard_area => move |_| {
    }
    ));

    // 请求和棋按钮(联机模式)
    let button_surrender = gtk::Button::with_label("请求和棋");
    button_reset.connect_clicked(clone!(
    @weak status_bar,
    @strong grid,
    @strong state,
    @weak chessboard_area,
    => move |_| {
    }
    ));

    // 逃跑按钮(联机模式)
    let button_exit = gtk::Button::with_label("逃跑");
    button_exit.connect_clicked(clone!(
    @weak stack,
    @strong connect_stage,
    @strong daemon_running,
    @strong cl_sender,
    => move |_| {
        cl_sender.send(NetworkEvent::LeaveRoom).unwrap();

        *connect_stage.lock().unwrap() = ConnectStage::No;
        *daemon_running.lock().unwrap() = false;

        stack.set_visible_child_name("title");
    }
    ));

    tool_bar_multiple_player.append(&button_undo);
    tool_bar_multiple_player.append(&button_surrender);
    tool_bar_multiple_player.append(&button_exit);

    chessboard_area.add_controller(click_reactor);

    let undo_bar = gtk::Box::builder()
        .hexpand(true)
        .build();

    let button_accept = gtk::Button::with_label("Accept");

    // 调用前需要先设置state.mode
    // 调用前不能锁定state，否则造成死锁
    let switch_tool_bar = clone!(
    @weak tool_bar_single_player,
    @weak tool_bar_multiple_player,
    @weak team_suggestion,
    @weam undo_bar,
    @strong state,
    => move |is_single_player| {
        undo_bar.set_visible(false);
        if is_single_player {
            tool_bar_single_player.set_visible(true);
            tool_bar_multiple_player.set_visible(false);
            team_suggestion.set_visible(false);
        } else {
            tool_bar_single_player.set_visible(false);
            tool_bar_multiple_player.set_visible(true);
            team_suggestion.set_visible(true);

            let state_ref = state.lock().unwrap();
            if let Mode::MultiplePlayer { ref my_team, .. } = &state_ref.mode {
                team_suggestion.set_label(&format!("你是 {}", my_team.as_str()));
            }
        }
    }
    );

    button_accept.connect_clicked(clone!(
    @strong switch_tool_bar,
    => move |_| {
        switch_tool_bar(false);

        
    }
    ));

    let box_up = gtk::Box::builder()
        .hexpand(true)
        .vexpand(true)
        .orientation(gtk::Orientation::Vertical)
        .build();

    box_up.append(&chessboard_area);
    box_up.append(&tool_bar_single_player);
    box_up.append(&tool_bar_multiple_player);
    box_up.append(&undo_bar);

    game_page.append(&box_up);
    game_page.append(&status_bar);
    game_page.append(&team_suggestion);





    let conn_status_bar = gtk::Label::builder()
        .label(STATUS_BAR_INITIAL_TEXT)
        .hexpand(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    // 房间页面
    let room_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(80)
        .vexpand(true)
        .hexpand(true)
        .build();

    let box_up = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .vexpand(true)
        .build();

    let room_owner_label = gtk::Label::new(Some(""));
    let room_player_label = gtk::Label::new(Some(""));

    box_up.append(&room_owner_label);
    box_up.append(&room_player_label);

    let tools_wait = gtk::CenterBox::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();

    let button_exit = gtk::Button::with_label("返回");
    button_exit.connect_clicked(clone!(
    @weak stack,
    @weak room_owner_label,
    @weak room_player_label,
    @weak conn_status_bar,
    @strong cl_sender,
    @strong daemon_running,
    @strong discover,
    @strong connect_stage,
    => move |_| {
        let mut connect_stage_ref = connect_stage.lock().unwrap();
        if let ConnectStage::Waiting { role, .. } = *connect_stage_ref {
            match role {
                Role::Owner => {
                    cl_sender.send(NetworkEvent::RoomDisbanded).unwrap();
                },

                Role::Player | Role::Visitor => {
                    cl_sender.send(NetworkEvent::LeaveRoom).unwrap();
                },

                _ => {},
            }
        }
        conn_status_bar.set_label(STATUS_BAR_INITIAL_TEXT);

        stack.set_visible_child_name("connect");
        room_owner_label.set_label("");
        room_player_label.set_label("");

        *connect_stage_ref = ConnectStage::No;
        *discover.lock().unwrap() = DiscoverState::Continue;
        *daemon_running.lock().unwrap() = false;
    }
    ));

    let switch_tool_bar_copy = switch_tool_bar.clone();
    let button_prepare = gtk::Button::with_label("");
    button_prepare.connect_clicked(clone!(
    @weak stack,
    @weak status_bar,
    @weak room_player_label,
    @weak room_owner_label,
    @strong connect_stage,
    @strong cl_sender,
    @strong state,
    @strong grid,
    => move |button_prepare| {
        let mut connect_stage_ref = connect_stage.lock().unwrap();
        if let ConnectStage::Waiting { ref mut prepared, role, .. } = (*connect_stage_ref).clone() {
            if role == Role::Owner && *prepared {
                cl_sender.send(NetworkEvent::StartGame).unwrap();

                // 开始游戏
                room_owner_label.set_label("");
                room_player_label.set_label("");

                let mut state_ref = state.lock().unwrap();
                state_ref.history.clear();
                state_ref.frozen = false;
                state_ref.current_team = Team::Black;
                state_ref.mode = Mode::MultiplePlayer {
                    my_team: Team::Black,
                    peer_name: if let ConnectStage::Waiting { ref opponent_name, .. } = *connect_stage_ref {
                        opponent_name.as_ref().unwrap().clone()
                    } else {
                        unreachable!();
                    },
                };
                status_bar.set_label(STATUS_BAR_INITIAL_TEXT);
                drop(state_ref);

                switch_tool_bar_copy(false);

                grid.borrow_mut().clear();

                *connect_stage_ref = ConnectStage::Gaming;

                stack.set_visible_child_name("game");
            } else if role == Role::Player {
                if let ConnectStage::Waiting { ref mut prepared, .. } = *connect_stage_ref {
                    // 切换准备
                    *prepared = !*prepared;
                    cl_sender.send(NetworkEvent::SetPrepared(*prepared)).unwrap();
                    if *prepared {
                        button_prepare.set_label("取消准备");
                    } else {
                        button_prepare.set_label("准备");
                    }
                } else {
                    unreachable!();
                }
            }
        }
    }
    ));

    tools_wait.set_start_widget(Some(&button_exit));
    tools_wait.set_end_widget(Some(&button_prepare));

    room_page.append(&box_up);
    room_page.append(&tools_wait);





    // 连接页面
    let connect_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();

    // 容纳列表和按钮的Box
    let box1 = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();

    // 搜索到的连接列表
    let connection_list = gtk::ListBox::builder()
        .hexpand(true)
        .vexpand(true)
        .selection_mode(gtk::SelectionMode::Single)
        .show_separators(true)
        .build();

    // 地址输入框
    let address_input = gtk::Text::builder()
        .hexpand(true)
        .editable(true)
        .placeholder_text("或是直接在这输入地址")
        .build();

    #[cfg(debug_assertions)]
    address_input.buffer().set_text("[::1]:12001");

    // Your name.
    let name_input = gtk::Text::builder()
        .hexpand(true)
        .editable(true)
        .placeholder_text("君の名は")
        .build();

    /*let conn_status_bar = gtk::Label::builder()
        .label(STATUS_BAR_INITIAL_TEXT)
        .hexpand(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();*/

    let update_status_bar = upsb.clone();
    let switch_tool_bar_copy = switch_tool_bar.clone();
    // 处理网络事件
    event_receiver.attach(None, clone!(
    @weak conn_status_bar,
    @weak status_bar,
    @weak connect_page,
    @weak room_owner_label,
    @weak room_player_label,
    @weak win,
    @weak stack,
    @weak name_input,
    @weak button_prepare,
    @strong last_pong,
    @strong connect_stage,
    @strong event_sender,
    @strong discover,
    @strong daemon_running,
    @strong grid,
    @strong state,
    => @default-return glib::Continue(true),
    move |event| {
        dbg!(&event);

        let myname = name_input.buffer().text().as_str().to_string();

        let mut connect_stage_ref = connect_stage.lock().unwrap();

        if let NetworkEvent::Pong {..} = event {
            *last_pong.lock().unwrap() = Instant::now();
            return glib::Continue(true);
        } else if let NetworkEvent::Ping {..} = event {
            event_sender.send(NetworkEvent::Pong { send_time: SystemTime::now() }).unwrap();
            return glib::Continue(true);
        }

        if *connect_stage_ref == ConnectStage::No {
            match event {
                NetworkEvent::Error(err) => {
                    conn_status_bar.set_label(&format!("网络错误:{}", err));
                    status_bar.set_label(&format!("网络错误:{}", err));
                    connect_page.set_sensitive(true);
                    *discover.lock().unwrap() = DiscoverState::Continue;
                    *daemon_running.lock().unwrap() = false;
                },

                NetworkEvent::EnterPermitted { name, role } => {
                    stack.set_visible_child_name("room");
                    *discover.lock().unwrap() = DiscoverState::Stop;
                    conn_status_bar.set_label("成功连接");
                    status_bar.set_label(STATUS_BAR_INITIAL_TEXT);
                    connect_page.set_sensitive(true);

                    room_owner_label.set_label(&format!("房主(黑方)    {name}"));
                    room_player_label.set_label(&format!("玩家(白方)    {myname}"));
                    button_prepare.set_label("准备");
                    *connect_stage_ref = ConnectStage::Waiting {
                        role,
                        opponent_name: Some(name),
                        prepared: false,
                    };
                },

                NetworkEvent::RoomIsFull => {
                    *discover.lock().unwrap() = DiscoverState::Continue;
                    *daemon_running.lock().unwrap() = false;
                    conn_status_bar.set_label("加入失败：房间已满");
                    status_bar.set_label("加入失败：房间已满");
                    connect_page.set_sensitive(true);
                },

                _ => {}
            }
        } else if let ConnectStage::Waiting { role, .. } = (*connect_stage_ref).clone() {
            match event {
                NetworkEvent::Error(msg) => {
                    stack.set_visible_child_name("connect");

                    room_owner_label.set_label("");
                    room_player_label.set_label("");
                    conn_status_bar.set_label(&format!("错误: {msg}"));

                    *connect_stage_ref = ConnectStage::No;
                    *discover.lock().unwrap() = DiscoverState::Continue;
                    *daemon_running.lock().unwrap() = false;
                },

                NetworkEvent::RoomDisbanded if role != Role::Owner => {
                    let msgbox = gtk::MessageDialog::builder()
                        .text("房间已散伙")
                        .buttons(gtk::ButtonsType::Ok)
                        .transient_for(&win)
                        .message_type(gtk::MessageType::Info)
                        .modal(true)
                        .build();
                    msgbox.present();

                    stack.set_visible_child_name("title");
                    *daemon_running.lock().unwrap() = false;
                    *connect_stage_ref = ConnectStage::No;
                },

                NetworkEvent::RoomDisbanded => {
                    cl_sender.send(NetworkEvent::Error("不是房主你散个屁的伙".to_string())).unwrap();
                },

                NetworkEvent::LeaveRoom if role == Role::Owner => {
                    if let ConnectStage::Waiting { ref mut opponent_name, ref mut prepared, .. } = *connect_stage_ref {
                        *opponent_name = None;
                        *prepared = false;
                    } else {
                        unreachable!();
                    }
                    room_player_label.set_label("等待加入...");
                },

                NetworkEvent::EnterRoom { name, .. } if role == Role::Owner => {
                    cl_sender.send(NetworkEvent::EnterPermitted {
                        role: Role::Player,
                        name: String::clone(&myname),
                    }).unwrap();

                    if let ConnectStage::Waiting { ref mut opponent_name, .. } = *connect_stage_ref {
                        *opponent_name = Some(String::clone(&name));
                    } else {
                        unreachable!();
                    }

                    room_player_label.set_label(&format!("玩家(白方)    {name}"));
                },

                NetworkEvent::SetPrepared(val) if role == Role::Owner => {
                    match *connect_stage_ref {
                        ConnectStage::Waiting { ref mut prepared, .. } => {
                            *prepared = val;
                        },
                        _ => unreachable!(),
                    }

                    if val {
                        button_prepare.set_label("开始游戏(已准备)");
                    } else {
                        button_prepare.set_label("开始游戏(未准备)");
                    }
                },

                NetworkEvent::StartGame if role == Role::Player => {
                    room_owner_label.set_label("");
                    room_player_label.set_label("");
                    status_bar.set_label(STATUS_BAR_INITIAL_TEXT);

                    let mut state_ref = state.lock().unwrap();
                    state_ref.history.clear();
                    state_ref.frozen = false;
                    state_ref.current_team = Team::Black;
                    state_ref.mode = Mode::MultiplePlayer {
                        my_team: Team::White,
                        peer_name: if let ConnectStage::Waiting { ref opponent_name, .. } = *connect_stage_ref {
                            opponent_name.as_ref().unwrap().clone()
                        } else {
                            unreachable!();
                        },
                    };
                    drop(state_ref);

                    switch_tool_bar_copy(false);

                    grid.borrow_mut().clear();

                    *connect_stage_ref = ConnectStage::Gaming;

                    stack.set_visible_child_name("game");
                },

                _ => {},
            }
        } else if *connect_stage_ref == ConnectStage::Gaming {
            match event {
                NetworkEvent::Error(msg) => {
                    status_bar.set_label(&format!("Fatal Error(Connection has been closed): {msg}"));

                    *connect_stage_ref = ConnectStage::No;
                    state.lock().unwrap().frozen = true;
                    *daemon_running.lock().unwrap() = false;
                },

                NetworkEvent::LeaveRoom => {
                    status_bar.set_label("对方跑了！");
                    *connect_stage_ref = ConnectStage::No;
                    state.lock().unwrap().frozen = true;
                    *daemon_running.lock().unwrap() = false;
                },

                NetworkEvent::PutChess {x, y} => {
                    let mut state_ref = state.lock().unwrap();
                    if let Mode::MultiplePlayer { ref my_team, .. } = state_ref.mode {
                        if *my_team != state_ref.current_team {
                            let mut grid_ref = grid.borrow_mut();

                            let mut target_opt = grid_ref.at_mut(x as isize, y as isize);
                            if let Some(ref mut target) = target_opt {
                                if target.chess.is_none() {
                                    state_ref.history.push((x as isize, y as isize));
                                    target.chess = Some(state_ref.current_team);
                                    state_ref.current_team.set_opposite();
                                    update_status_bar(state_ref.current_team);
                                    chessboard_area.queue_draw();
                                } else {
                                    state_ref.frozen = true;
                                    status_bar.set_label("错误: 对方下棋，但那里已经有棋了");
                                    *daemon_running.lock().unwrap() = false;
                                    cl_sender.send(NetworkEvent::Error("Cannot cover a chess".to_owned())).unwrap();
                                }
                            } else {
                                state_ref.frozen = true;
                                status_bar.set_label("错误: 对方下了个无效坐标");
                                *daemon_running.lock().unwrap() = false;
                                cl_sender.send(NetworkEvent::Error("Invalid position".to_owned())).unwrap();
                            }
                        } else {
                            state_ref.frozen = true;
                            status_bar.set_label("错误: 对方下棋，但不是TA的回合");
                            *daemon_running.lock().unwrap() = false;
                            cl_sender.send(NetworkEvent::Error("You were trying to put a chess while it was not your round".to_owned())).unwrap();
                        }
                    } else {
                        unreachable!();
                    }
                },

                NetworkEvent::UndoRequest => {
                },

                _ => {},
            }
        }
        glib::Continue(true)
    }
    ));

    // 对地址进行连接。该闭包会召唤新线程处理连接
    let do_connect = clone!(
    @weak conn_status_bar,
    @weak stack,
    @strong event_sender,
    @strong connect_stage,
    @strong discover,
    @strong last_pong,
    @strong last_ping,
    @strong daemon_running,
    @strong cl_receiver,
    @weak connect_page,
    @weak name_input,
    => move |address: SocketAddr| {
        conn_status_bar.set_label(&format!("正在连接到{}", address));
        *discover.lock().unwrap() = DiscoverState::Pause;
        *daemon_running.lock().unwrap() = true;
        connect_page.set_sensitive(false);
        *last_pong.lock().unwrap() = Instant::now();
        *last_ping.lock().unwrap() = Instant::now();

        let buf = name_input.buffer();
        if buf.bytes() == 0 || buf.bytes() > 100 {
            buf.set_text(format!("player{}", rand::thread_rng().gen_range(0..100000)));
        }
        let myname = buf.text().as_str().to_string();

        thread::spawn(move || {
            // 这个线程只负责建立TCP连接与持续接收网络数据

            let result = TcpStream::connect_timeout(&address, Duration::from_millis(20000));
            if let Err(err) = result {
                event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                return;
            }

            // 连接成功后持续接收网络数据
            // 后续应用层的消息处理将由event_receiver完成
            let mut stream = result.unwrap();
            let mut buf = [0u8; 2048];
            let mut bytes_available = 0usize;

            stream.set_read_timeout(Some(Duration::from_millis(80))).unwrap();

            macro_rules! _unwrap {
                ($result:expr) => {
                    if let Err(err) = $result {
                        dbg!(&err);
                        event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                        return;
                    }
                }
            }

            // 发送请求加入的消息
            let packet = NetworkEvent::EnterRoom { role: Role::Player, name: myname }.to_u8_vec();
            _unwrap!(stream.write(packet.as_slice()));

            while *daemon_running.lock().unwrap() == true {
                // 检测超时
                let lpe = last_pong.lock().unwrap().elapsed();
                let connect_stage_ref = connect_stage.lock().unwrap();
                if *connect_stage_ref == ConnectStage::No && lpe > Duration::from_secs(15) {
                    event_sender.send(NetworkEvent::Error("Entering timed out".to_string())).unwrap();
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                } else if let ConnectStage::Waiting {..} = *connect_stage_ref {
                    if lpe > Duration::from_secs(30) {
                        event_sender.send(NetworkEvent::Error("Timed out".to_string())).unwrap();
                        let _ = stream.shutdown(Shutdown::Both);
                        return;
                    }
                }
                drop(lpe);

                // 每5秒ping一次
                let last_ping_el = last_ping.lock().unwrap().elapsed();
                if last_ping_el >= Duration::from_secs(5) {
                    *last_ping.lock().unwrap() = Instant::now();
                    let packet = NetworkEvent::Ping { send_time: SystemTime::now() }.to_u8_vec();
                    _unwrap!(stream.write(packet.as_slice()));
                }

                if bytes_available >= 2048 {
                    event_sender.send(NetworkEvent::Error("Buffer is overflowing".to_owned())).unwrap();
                }

                let result = stream.read(&mut buf[bytes_available..]);
                let bytes_new = match result {
                    Ok(b) => b,
                    Err(err) => {
                        use std::io::ErrorKind as EK;
                        match err.kind() {
                            EK::WouldBlock | EK::TimedOut => 0,

                            _ => {
                                event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                                *daemon_running.lock().unwrap() = false;
                                return;
                            },
                        }
                    },
                };

                bytes_available += bytes_new;

                while let Some((event, length)) = NetworkEvent::from_buffer(&buf[0..bytes_available]) {
                    // 发送接收到的数据
                    event_sender.send(event).unwrap();

                    // 将已解析的数据忽略
                    if length == bytes_available {
                        bytes_available = 0;
                    } else if length == 0 {
                        break;
                    } else {
                        let rest_data = buf[length..bytes_available].to_owned();
                        buf[0..(bytes_available - length)].clone_from_slice(rest_data.as_slice());
                        bytes_available -= length;
                    }
                }

                use std::sync::mpsc::RecvTimeoutError;
                let r = cl_receiver.lock().unwrap();
                match r.recv_timeout(Duration::from_millis(50)) {
                    Ok(event) => {
                        println!("Time at {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
                        println!("Send event {event:?} to peer");
                        _unwrap!(stream.write(event.to_u8_vec().as_slice()));
                    },

                    Err(RecvTimeoutError::Timeout) => (),

                    Err(RecvTimeoutError::Disconnected) => {
                        event_sender.send(NetworkEvent::Error("Channel unexpectedly disconnected".to_string())).unwrap();
                        return;
                    },
                }
            }
        });
    }
    );

    let button_connect_address = gtk::Button::with_label("连接到这个地址");
    button_connect_address.connect_clicked(clone!(
    @weak address_input, @weak conn_status_bar => move |_| {
        let tmp = address_input.buffer().text();
        let address_str = tmp.as_str();
        match address_str.parse::<SocketAddr>() {
            Ok(address) => {
                do_connect.clone()(address);
            },
            Err(err) => {
                conn_status_bar.set_label(&format!("地址格式错误:{}", err));
            },
        }
    }
    ));

    // 返回主页面按钮
    let button_exit = gtk::Button::with_label("返回主页面");
    button_exit.connect_clicked(clone!(
    @weak stack, @strong discover => move |_| {
        *discover.lock().unwrap() = DiscoverState::Stop;

        stack.set_visible_child_name("title");
    }
    ));

    // 创建房间按钮
    let button_create_room = gtk::Button::with_label("创建房间");
    button_create_room.connect_clicked(clone!(
    @weak stack,
    @weak room_owner_label,
    @weak room_player_label,
    @weak name_input,
    @weak button_prepare,
    @strong daemon_running,
    @strong last_pong,
    @strong last_ping,
    @strong event_sender,
    @strong connect_stage,
    @strong discover,
    @strong cl_receiver,
    => move |_| {
        *discover.lock().unwrap() = DiscoverState::Stop;
        *connect_stage.lock().unwrap() = ConnectStage::Waiting {
            role: Role::Owner,
            opponent_name: None,
            prepared: false,
        };
        *daemon_running.lock().unwrap() = true;
        *last_pong.lock().unwrap() = Instant::now();
        *last_ping.lock().unwrap() = Instant::now();

        button_prepare.set_label("开始游戏");

        let buf = name_input.buffer();
        if buf.bytes() == 0 || buf.bytes() > 100 {
            buf.set_text(format!("player{}", rand::thread_rng().gen_range(0..100000)));
        }
        let myname = buf.text().as_str().to_string();

        room_owner_label.set_label(&format!("房主(黑方):    {myname}"));
        room_player_label.set_label("等待加入...");

        // 召唤新线程处理连接
        thread::spawn(clone!(
        @strong cl_receiver,
        @strong connect_stage,
        @strong last_ping,
        @strong last_pong,
        @strong event_sender,
        @strong daemon_running,
        => move || {
            // 监听端口
            let listener = match TcpListener::bind("[::]:12001".parse::<SocketAddr>().unwrap()) {
                Ok(l) => l,
                Err(err) => {
                    event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                    return;
                },
            };

            let mut buf = [0u8; 2048];
            let mut bytes_available = 0usize;

            macro_rules! _unwrap {
                ($result:expr) => {{
                    match $result {
                        Ok(r) => r,
                        Err(err) => {
                            dbg!(&err);
                            event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                            return;
                        }
                    }
                }}
            }

            macro_rules! loop_accept {
                (@onerror $errid:ident => $onerror:block$(, @none_cond $nc:block)?) => {
                    loop {
                        use std::io::ErrorKind as EK;
                        match listener.accept() {
                            Ok(data) => break Some(data),
                            Err($errid) => {
                                match $errid.kind() {
                                    EK::TimedOut | EK::WouldBlock => (),
                                    _ => {
                                        $onerror;
                                    },
                                }
                            },
                        }
                        if !*daemon_running.lock().unwrap() {
                            return;
                        }
                        $(if $nc {
                            break None;
                        })?
                        thread::sleep(Duration::from_millis(80));
                    }
                }
            }

            listener.set_nonblocking(true).unwrap();
            let mut error_just_now = false;
            let mut need_reconnect = false;
            let (mut stream, _) = loop_accept!(@onerror err => {
                event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                return;
            }).unwrap();
            stream.set_read_timeout(Some(Duration::from_millis(80))).unwrap();

            while *daemon_running.lock().unwrap() {
                let connect_stage_ref = connect_stage.lock().unwrap();
                if let ConnectStage::Waiting {ref opponent_name, ..} = *connect_stage_ref {
                    if opponent_name.is_none() && need_reconnect {
                        if let Some((stream_tmp, _)) = loop_accept!(@onerror err => {
                            event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                            return;
                        }, @none_cond { opponent_name.is_none() }) {
                            stream = stream_tmp;
                            need_reconnect = false;
                        }
                    } else if opponent_name.is_some() {
                        need_reconnect = true;
                    }
                }

                if let Ok((mut stream_tmp, _)) = listener.accept() {
                    let _ = stream_tmp.write(NetworkEvent::RoomIsFull.to_u8_vec().as_slice());
                    let _ = stream_tmp.shutdown(Shutdown::Both);
                }

                // 检测超时
                let lpe = last_pong.lock().unwrap().elapsed();
                if *connect_stage_ref == ConnectStage::No && lpe > Duration::from_secs(15) {
                    event_sender.send(NetworkEvent::Error("Entering timed out".to_string())).unwrap();
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                } else if let ConnectStage::Waiting {..} = *connect_stage_ref {
                    if lpe > Duration::from_secs(30) {
                        event_sender.send(NetworkEvent::Error("Timed out".to_string())).unwrap();
                        let _ = stream.shutdown(Shutdown::Both);
                        return;
                    }
                }

                // 每5秒ping一次
                let last_ping_el = last_ping.lock().unwrap().elapsed();
                if last_ping_el >= Duration::from_secs(5) {
                    *last_ping.lock().unwrap() = Instant::now();
                    let packet = NetworkEvent::Ping { send_time: SystemTime::now() }.to_u8_vec();
                    if let Err(err) = stream.write(packet.as_slice()) {
                        if error_just_now {
                            dbg!(&err);
                            event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                            return;
                        } else {
                            error_just_now = true;
                            continue;
                        }
                    }
                }

                if bytes_available >= 2048 {
                    event_sender.send(NetworkEvent::Error("Buffer is overflowing".to_owned())).unwrap();
                }

                let result = stream.read(&mut buf[bytes_available..]);
                let bytes_new = match result {
                    Ok(b) => b,
                    Err(err) => {
                        use std::io::ErrorKind as EK;
                        match err.kind() {
                            EK::WouldBlock | EK::TimedOut => 0,

                            _ => {
                                if error_just_now {
                                    event_sender.send(NetworkEvent::Error(err.to_string())).unwrap();
                                    *daemon_running.lock().unwrap() = false;
                                    return;
                                } else {
                                    error_just_now = true;
                                    continue;
                                }
                            },
                        }
                    },
                };
                error_just_now = false;

                bytes_available += bytes_new;

                while let Some((event, length)) = NetworkEvent::from_buffer(&buf[0..bytes_available]) {
                    // 发送接收到的数据
                    event_sender.send(event).unwrap();

                    // 将已解析的数据忽略
                    if length == bytes_available {
                        bytes_available = 0;
                    } else if length == 0 {
                        break;
                    } else {
                        let rest_data = buf[length..bytes_available].to_owned();
                        buf[0..(bytes_available - length)].clone_from_slice(rest_data.as_slice());
                        bytes_available -= length;
                    }
                }

                use std::sync::mpsc::RecvTimeoutError;
                let r = cl_receiver.lock().unwrap();
                match r.recv_timeout(Duration::from_millis(50)) {
                    Ok(event) => {
                        println!("Send event {event:?} to peer");
                        _unwrap!(stream.write(event.to_u8_vec().as_slice()));
                    },

                    Err(RecvTimeoutError::Timeout) => (),

                    Err(RecvTimeoutError::Disconnected) => {
                        event_sender.send(NetworkEvent::Error("Channel unexpectedly disconnected".to_string())).unwrap();
                        return;
                    },
                }
            }
        }
        ));

        stack.set_visible_child_name("room");
    }
    ));

    // 尝试连接按钮
    let button_connect = gtk::Button::with_label("连接");
    button_connect.connect_clicked(clone!(
    @weak connection_list => move |_| {
        //let row = connection_list.selected_row();
        //thread::spawn();
    }
    ));

    // 当无选择的时候禁用连接按钮
    connection_list.connect_unselect_all(clone!(
    @weak button_connect => move |_| {
        button_connect.set_sensitive(false);
    }
    ));

    // 有选择时启用连接按钮
    connection_list.connect_row_selected(clone!(
    @weak button_connect => move |_, _| {
        button_connect.set_sensitive(true);
    }
    ));

    let box_custom_addr = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .spacing(5)
        .build();

    let box2 = gtk::CenterBox::builder()
        .hexpand(true)
        .orientation(gtk::Orientation::Horizontal)
        .build();

    box2.set_start_widget(Some(&button_exit));
    box2.set_center_widget(Some(&button_create_room));
    box2.set_end_widget(Some(&button_connect));

    box_custom_addr.append(&address_input);
    box_custom_addr.append(&button_connect_address);

    box1.append(&connection_list);
    box1.append(&name_input);
    box1.append(&box_custom_addr);
    box1.append(&box2);

    connect_page.append(&box1);
    connect_page.append(&conn_status_bar);





    // 标题界面
    let title_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .spacing(50)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    let title = gtk::Label::builder()
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .margin_top(20)
        .margin_bottom(20)
        .label("gggggomoku")
        .build();

    let button_single_player = gtk::Button::with_label("单机游玩");

    let switch_tool_bar_copy = switch_tool_bar.clone();
    button_single_player.connect_clicked(clone!(
    @weak stack, @strong state, @strong grid, @weak status_bar => move |_| {
        let mut state_ref = state.lock().unwrap();
        state_ref.current_team = Team::Black;
        state_ref.history.clear();
        state_ref.mode = Mode::Singleplayer;
        state_ref.frozen = false;

        switch_tool_bar_copy(true);

        status_bar.set_label(STATUS_BAR_INITIAL_TEXT);

        grid.borrow_mut().clear();

        stack.set_visible_child_name("game");
    }
    ));

    let button_multiple_player = gtk::Button::with_label("联机游玩");

    button_multiple_player.connect_clicked(clone!(
    @weak stack,
    @weak button_connect,
    @weak connect_page,
    @weak conn_status_bar,
    @strong discover => move |_| {
        *discover.lock().unwrap() = DiscoverState::Continue;
        button_connect.set_sensitive(false);
        connect_page.set_sensitive(true);
        conn_status_bar.set_label(STATUS_BAR_INITIAL_TEXT);

        stack.set_visible_child_name("connect");
    }
    ));

    title_page.append(&title);
    title_page.append(&button_single_player);
    title_page.append(&button_multiple_player);


    stack.add_named(&title_page, Some("title"));
    stack.add_named(&game_page,  Some("game"));
    stack.add_named(&connect_page, Some("connect"));
    stack.add_named(&room_page, Some("room"));

    stack.set_visible_child_name("title");

    win.show();
}


fn get_a_good_adj()-> &'static str {
    static ADJS: OnceCell<Vec<&'static str>> = OnceCell::new();
    let adjs = ADJS.get_or_init(|| {
        vec![
            "击败了参与的99%的选手",
            "斯国一得斯",
            "真棒，太棒了",
            "Good job",
            "获胜了",
            " attempted to `CheatSheet::new()` Panicking at the brain █.c:█,1",
            "赢了，奖励艾草一包",
            "大获全胜。愣着啊w鼓掌干什么",
            "赢得了胜利，恭喜这个b...",
            "赢了！！！！！！！！！！",
            "五子连珠，召唤神龙",
            "当时觉得挺有意思的，就下了下，没想到真的赢了",
            "太美",
            "！",
        ]
    });
    let mut rng = rand::thread_rng();
    adjs[rng.gen_range(0..adjs.len())]
}



#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Team {
    White,

    #[default]
    Black,
}

impl Team {
    pub fn get_opposite(&self)-> Self {
        match *self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    pub fn set_opposite(&mut self) {
        *self = self.get_opposite();
    }

    pub fn as_str(&self)-> &'static str {
        match *self {
            Team::White => "白方",
            Team::Black => "黑方",
        }
    }
}



#[derive(Clone)]
struct Cell {
    /// None表示没有棋子
    /// Some(Team)表示有相应队伍的棋子
    pub chess: Option<Team>,
}



struct ChessboardGrid {
    m_vec: Vec<Cell>,
}

impl ChessboardGrid {
    pub fn new()-> Self {
        Self {
            m_vec: vec![Cell { chess: None }; 15 * 15],
        }
    }

    pub fn at(&self, x: isize, y: isize)-> Option<&Cell> {
        if x >= 15 || y >= 15 || x < 0 || y < 0 {
            None
        } else {
            Some(&self.m_vec[(y * 15 + x) as usize])
        }
    }

    pub fn at_mut(&mut self, x: isize, y: isize)-> Option<&mut Cell> {
        if x >= 15 || y >= 15 || x < 0 || y < 0 {
            None
        } else {
            Some(&mut self.m_vec[(y * 15 + x) as usize])
        }
    }

    pub fn clear(&mut self) {
        self.m_vec.iter_mut().for_each(|i| i.chess = None);
    }

    /// 检查是否有其中一方获胜
    ///
    /// 如果没有获胜则返回`None`，如果有一方获胜则返回`Some(Team)`获胜的队伍
    pub fn check_win(&self)-> Option<Team> {
        macro_rules! gen_check {
            ($current_chess:ident, $last_chess:ident, $counter:ident) => {
                if $current_chess.is_none() {
                    $last_chess = None;
                    $counter = 0;
                } else if $last_chess.is_none() {
                    $counter += 1;
                    $last_chess = $current_chess;
                } else {
                    if $current_chess.unwrap() == $last_chess.unwrap() {
                        $counter += 1;
                    } else {
                        $last_chess = $current_chess;
                        $counter = 1;
                    }
                }

                if $counter >= 5 {
                    return Some($current_chess.unwrap());
                }
            };
        }

        // 横向
        for y in 0..14 {
            let mut last_chess = None::<Team>;
            let mut counter = 0;
            for x in 0..14 {
                let current_chess = self.at(x, y).unwrap().chess;
                gen_check!(current_chess, last_chess, counter);
            }
        }

        // 纵向
        for x in 0..14 {
            let mut last_chess = None::<Team>;
            let mut counter = 0;
            for y in 0..14 {
                let current_chess = self.at(x, y).unwrap().chess;
                gen_check!(current_chess, last_chess, counter);
            }
        }

        // 斜向(/)
        for x in 0..15 {
            {
                let mut last_chess = None::<Team>;
                let mut counter = 0;
                for offset in 0.. {
                    if let Some(cell) = self.at(x - offset, offset) {
                        let current_chess = cell.chess;
                        gen_check!(current_chess, last_chess, counter);
                    } else {
                        break;
                    }
                }
            }
            {
                let mut last_chess = None::<Team>;
                let mut counter = 0;
                for offset in 0.. {
                    if let Some(cell) = self.at(x + offset, 14 - offset) {
                        let current_chess = cell.chess;
                        gen_check!(current_chess, last_chess, counter);
                    } else {
                        break;
                    }
                }
            }
        }

        // 斜向(\)
        for x in 0..15 {
            {
                let mut last_chess = None::<Team>;
                let mut counter = 0;
                for offset in 0.. {
                    if let Some(cell) = self.at(x + offset, offset) {
                        let current_chess = cell.chess;
                        gen_check!(current_chess, last_chess, counter);
                    } else {
                        break;
                    }
                }
            }
            {
                let mut last_chess = None::<Team>;
                let mut counter = 0;
                for offset in 0.. {
                    if let Some(cell) = self.at(x - offset, 14 - offset) {
                        let current_chess = cell.chess;
                        gen_check!(current_chess, last_chess, counter);
                    } else {
                        break;
                    }
                }
            }
        }

        None
    }
}

impl Default for ChessboardGrid {
    fn default()-> Self {
        Self::new()
    }
}



#[derive(Default)]
struct State {
    pub current_team: Team,
    pub history: Vec<(isize, isize)>,
    pub mode: Mode,
    pub frozen: bool,
}

#[derive(Default)]
enum Mode {
    #[default]
    Singleplayer,

    MultiplePlayer {
        peer_name: String,
        my_team: Team,
    },
}

impl Mode {
    pub fn is_single_player(&self)-> bool {
        match self {
            &Self::Singleplayer => true,
            &Self::MultiplePlayer {..} => false,
        }
    }

    pub fn is_multiple_player(&self)-> bool {
        !self.is_single_player()
    }
}
