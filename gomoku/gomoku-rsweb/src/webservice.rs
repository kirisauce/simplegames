use crate::game::*;

use std::path::PathBuf;
use std::fs::File;
use std::collections::HashMap;
use std::ops::*;
use std::thread;
use std::time::Duration;
use std::mem::swap;
use std::sync::{
    Arc,
    Mutex,
    MutexGuard,
    atomic::{
        AtomicBool,
        Ordering::*,    
    },
};

use uuid::Uuid;
use uuid::fmt::*;
use serde::Serialize;
use actix::{
    Actor,
    StreamHandler,
    AsyncContext,
    prelude::*,
};
use actix_web::{
    web,
    HttpResponse,
    HttpRequest,
    get,
};
use actix_web_actors::ws;



/// 游戏Session连接的Websocket
struct GameWs {
    pub smanager: &mut SessionManager,
}

impl Actor for GameWs {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for GameWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(ping)) => {
                println!("Ping from {:?}", ctx.address());
                return ctx.pong(&ping);
            },
            Ok(ws::Message::Pong(_)) => (),
            Ok(ws::Message::Text(text)) => {
                println!("Text from {:?}", ctx.address());
                return ctx.text(text);
            },
            Ok(ws::Message::Binary(_)) => {
                ctx.close(ws::CloseReason {
                    code: ws::CloseCode::Protocol,
                    description: Some("Unexpected binary data".to_string()),
                })
            },
            Ok(ws::Message::Close(reason)) => {
                let conns = self.smanager.get_connections();
                if conns.contains(ctx.address()) {
                }
            },
            Err(err) => {
                ctx.close(ws::CloseReason {
                    code: ws::CloseCode::Protocol,
                    description: 
                })
            },
        }
    }
}



/// 描述一个连接
pub struct Connection {
    m_associated_session: Uuid,
}

/// 负责管理游戏会话的结构体
///
/// 会启动一个名为`SessionGCDaemon`的
/// 线程，负责清理无响应的Session。
///
/// 该线程每5分钟检查一次，如果发现超
/// 过10分钟无人游玩的Session则清理之。
/// 因此，无人游玩的Session最长存活时间在10~15分钟之间。
///
/// 在结构体被Drop时Daemon会在5秒内自动退出。
pub struct SessionManager {
    /// 存储所有游戏
    m_sessions: Arc<Mutex<HashMap<Uuid, GameSession>>>,

    /// 存储所有连接
    m_connections: Arc<Mutex<HashMap<Addr<GameWs>, Connection>>>,

    /// Daemon的Handler
    m_daemon: Option<thread::JoinHandle<()>>,

    /// 控制Daemon运行的变量
    m_condition: Arc<AtomicBool>,
}

impl SessionManager {
    pub fn new()-> Self {
        let sessions = Arc::new(Mutex::new(HashMap::<Uuid, GameSession>::with_capacity(15)));
        let cond = Arc::new(AtomicBool::new(true));
        let sessions_d = Arc::clone(&sessions);
        let c = Arc::clone(&cond);
        let d = thread::Builder::new()
            .name("SessionGCDaemon".to_string())
            .spawn(move || {
            let mut flag = false;
            loop {
                {
                    let mut sessions = sessions_d.lock().unwrap();
                    sessions.retain(|_, v| !v.check_timeout());

                    println!("Checked. Now sessions: {}", sessions.len());

                    // 每5分钟检查一次
                }
                for _ in 0..60 {
                    if !c.load(Acquire) {
                        flag = true;
                        break;
                    }
                    thread::sleep(Duration::from_secs(5));
                }
                if flag {
                    break;
                }
            }
        }).unwrap();
        Self {
            m_sessions: Arc::clone(&sessions),
            m_connections: Arc::new(Mutex::new(HashMap::new())),
            m_daemon: Some(d),
            m_condition: cond,
        }
    }

    pub fn get_sessions(&self)-> MutexGuard<HashMap<Uuid, GameSession>> {
        self.m_sessions.lock().unwrap()
    }

    pub fn get_connections(&self)-> MutexGuard<HashMap<Uuid, Connection>> {
        self.m_connections.lock().unwrap()
    }

    pub fn create_session(&self)-> Uuid {
        let s = GameSession::new();
        let id = s.get_game_uuid();
        let mut sessions = self.m_sessions.lock().unwrap();
        sessions.insert(id, s);
        id
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        self.m_condition.store(false, Release);
        let mut d = None;
        swap(&mut self.m_daemon, &mut d);
        if d.is_some() {
            d.unwrap().join().unwrap();
        }
    }
}



#[derive(Serialize)]
pub struct Message<T> {
    code: i32,
    msg: String,
    data: Option<T>,
}



#[cfg(debug_assertions)]
macro_rules! get_file {
    ($path:expr) => {{
        use std::io::Read;
        let path: PathBuf = ["src", $path].iter().collect();
        let mut f = File::open(path).expect("Cannot open file");
        let mut s = String::new();
        f.read_to_string(&mut s).expect("Failed to read file");
        s
    }}
}



#[cfg(not(debug_assertions))]
macro_rules! get_file {
    ($path:expr) => {
        std::include_str!($path)
    }
}



#[get("/index")]
pub async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/html"))
        .body(get_file!("index.html"))
}

#[get("/singleplayer")]
pub async fn singleplayer()-> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/html"))
        .body(get_file!("singleplayer.html"))
}

#[get("/multiplayer")]
pub async fn multiplayer()-> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/html"))
        .body(get_file!("multiplayer.html"))
}

#[get("/api/{api_name}")]
pub async fn api(
    smanager: web::Data<Mutex<SessionManager>>,
    api_name: web::Path<String>,
    req: HttpRequest,
    stream: web::Payload
    )-> HttpResponse {

    let api_name = api_name.into_inner();
    let smanager = smanager.lock().unwrap();
    let mut resp = HttpResponse::Ok();
    resp.insert_header(("Content-Type", "application/json"));
    match &api_name[..] {

        /// 新建一个Session
        "create_game" => {
            let id = smanager.create_session();
            let sessions = smanager.get_sessions();
            let session = sessions.get(&id).unwrap();
            #[derive(Serialize)]
            struct GameMetadata {
                uuid: Hyphenated,
            }
            resp.body(serde_json::to_string(&Message {
                code: 200,
                msg: "Game created".to_string(),
                data: Some(GameMetadata {
                    uuid: session.get_game_uuid().hyphenated(),
                }),
            }).unwrap())
        },

        /// 获取所有Session的UUID
        "get_all_games" => {
            #[derive(Serialize)]
            struct GameList {
                games: Vec<Hyphenated>,
            }
            resp.body(serde_json::to_string(&Message {
                code: 200,
                msg: "Ok".to_string(),
                data: Some(GameList {
                    games: smanager.get_sessions().keys().map(|v| v.hyphenated()).collect(),
                }),
            }).unwrap())
        },

        /// 获取Session数量
        "get_sessions_count" => {
            #[derive(Serialize)]
            struct SessionCount {
                count: usize,
            }
            resp.body(serde_json::to_string(&Message {
                code: 200,
                msg: "Ok".to_string(),
                data: Some(SessionCount {
                    count: smanager.get_sessions().len(),
                }),
            }).unwrap())
        },

        /// 获取推荐的Session
        "get_recommended_sessions" => {
            #[derive(Serialize)]
            struct SessionInfo {
                uuid: Hyphenated,
            }
            #[derive(Serialize)]
            struct RecommendedSessions {
                sessions: Vec<SessionInfo>,
            }
            let s = smanager.get_sessions();
            let mut sessions = Vec::<SessionInfo>::new();
            if s.len() <= 10 {
                sessions = s.iter().map(|v| {
                    SessionInfo {
                        uuid: v.1.get_game_uuid().hyphenated(),
                    }
                }).collect();
            }
            resp.body(serde_json::to_string(&Message {
                code: 200,
                msg: "Ok".to_string(),
                data: Some(RecommendedSessions {
                    sessions: sessions,
                }),
            }).unwrap())
        },

        /// 生成随机的UUID
        "get_random_uuid" => {
            #[derive(Serialize)]
            struct UuidResponse {
                uuid: Hyphenated,
            }
            resp.body(serde_json::to_string(&Message {
                code: 200,
                msg: "Ok".to_string(),
                data: Some(UuidResponse {
                    uuid: Uuid::new_v4().hyphenated(),
                }),
            }).unwrap())
        },

        /// 建立Websocket连接
        "connect" => {
            let resp = ws::start(GameWs {
                smanager: Arc::clone(&smanager),
            }, &req, stream).unwrap();
            println!("Websocket Response: {:?}", resp);
            resp
        },

        _ => resp.body(r#"{"code": 404, "msg": "API not found"}"#),
    }
} // fn api();

pub async fn error() -> HttpResponse {
    HttpResponse::NotFound()
        .insert_header(("Content-Type", "text/html"))
        .body(get_file!("404.html"))
}

#[get("/vue.global.js")]
pub async fn vue()-> HttpResponse {
    let mut r = HttpResponse::Ok();
    r.insert_header(("Content-Type", "application/javascript"));
    if cfg!(debug_assertions) {
        r.body(include_str!("vue-3.2.47.global.js"))
    } else {
        r.body(include_str!("vue-3.2.47.global.prod.js"))
    }
}
