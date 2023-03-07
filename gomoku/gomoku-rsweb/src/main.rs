use std::sync::Mutex;

use actix_web::{
    HttpServer,
    App,
    web,
};

mod webservice;
use webservice::*;
mod game;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let data = web::Data::new(Mutex::new(SessionManager::new()));
    HttpServer::new(move || {
        App::new()
        .app_data(data.clone())
        .service((
            web::redirect("/", "/index"),
            index,
            singleplayer,
            multiplayer,
            vue,
            api,
        ))
        .default_service(web::to(error))
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await
}
