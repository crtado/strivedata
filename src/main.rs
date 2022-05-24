use actix_files::Files;
use actix_web::{
    error,
    get,
    middleware,
    web,
    Error as AWError,
    App,
    HttpRequest,
    HttpResponse,
    HttpServer,
};
use r2d2_sqlite::{self, SqliteConnectionManager};
use tera::Tera;

mod db;
use db::Pool;

fn error_response(e: &str, tmpl: &web::Data<Tera>) -> Result<HttpResponse, AWError> {
    let mut ctx = tera::Context::new();
    ctx.insert("error", e);
    let s = tmpl
        .render("error.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Tera Error: {}", e)))?;
    return Ok(HttpResponse::Ok().content_type("text/html").body(s));
}

#[get("/about")]
async fn about(
    _req: HttpRequest,
    tmpl: web::Data<Tera>,
    _db: web::Data<Pool>,
) -> Result<HttpResponse, AWError> {
    let ctx = tera::Context::new();
    let s = tmpl
        .render("about.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Tera Error: {}", e)))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[get("/")]
async fn index(
    _req: HttpRequest,
    tmpl: web::Data<Tera>,
    db: web::Data<Pool>,
) -> Result<HttpResponse, AWError> {
    let result = db::most_matches_played(&db).await;

    let mut ctx = tera::Context::new();

    match result {
        Ok(r) => {
            ctx.insert("total_matches", &r);
        },
        Err(err) => {
            log::error!("Error getting most matches: {}", err);
        },
    };

    let result = db::recent_winners(&db).await;

    match result {
        Ok(r) => {
            ctx.insert("recent_winners", &r);
        },
        Err(err) => {
            log::error!("Error getting winrates: {}", err);
        },
    }

    let s = tmpl
        .render("index.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Tera error:{}", e)))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[get("/search/{tag}")]
async fn search(
    req: HttpRequest,
    tmpl: web::Data<Tera>,
    db: web::Data<Pool>,
) -> Result<HttpResponse, AWError> {
    let tag: &str = req.match_info().get("tag").unwrap();

    let result = db::players_by_tag(&db, tag.to_string())
        .await
        .unwrap_or_default();

    if result.len() == 1 {
        return Ok(HttpResponse::Found()
            .append_header(("location", format!("/player/{}", &result[0].id)))
            .finish());
    }

    let mut ctx = tera::Context::new();

    let s = match result.len() {
        0 => {
            return error_response("No results", &tmpl);
        },
        _ => {
            ctx.insert("players", &result);
            tmpl.render("search.html", &ctx)
                .map_err(|e| error::ErrorInternalServerError(format!("Tera Error: {}", e)))?
        },
    };
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[get("/player/{id}")]
async fn player(
    req: HttpRequest,
    tmpl: web::Data<Tera>,
    db: web::Data<Pool>,
) -> Result<HttpResponse, AWError> {
    let id: i32 = req
        .match_info()
        .get("id")
        .unwrap()
        .parse()
        .unwrap_or_default();

    let result = db::player_by_id(&db, id).await;

    let mut ctx = tera::Context::new();

    match result {
        Ok(row) => {
            ctx.insert("player", &row);
        },
        Err(err) => {
            log::error!("{}", err);
            return error_response("Player does not exist", &tmpl);
        },
    }

    // player metrics as vec[sets won, sets total, matches won, matches total]
    let result = db::player_metrics(&db, id).await;

    match result {
        Ok(row) => {
            ctx.insert("metrics", &row);
        },
        Err(err) => {
            log::error!("{}", err);
            return error_response("Unable to fetch player metrics", &tmpl);
        },
    }

    // player standings as tournament name, placement integer
    let result = db::player_standings(&db, id).await;

    match result {
        Ok(rows) => {
            ctx.insert("standings", &rows);
        },
        Err(err) => {
            log::error!("{}", err);
            return error_response("Unable to fetch player standings", &tmpl);
        },
    }

    // player results
    let result = db::player_sets(&db, id).await;

    match result {
        Ok(rows) => {
            ctx.insert("sets", &rows);
        },
        Err(err) => {
            log::error!("{}", err);
            return error_response("Unable to fetch player sets", &tmpl);
        },
    }

    let s = tmpl
        .render("player.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Tera Error: {}", e)))?;

    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match cfg!(debug_assertions) {
        false => env_logger::init_from_env(env_logger::Env::new().default_filter_or("warning")),
        true => env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug")),
    }

    let manager = match cfg!(debug_assertions) {
        false => SqliteConnectionManager::file("/data/db/test.db"),
        true => SqliteConnectionManager::file("test.db"),
    };

    let pool = Pool::new(manager).unwrap();

    log::warn!("starting http server at http://localhost:8080");

    HttpServer::new(move || {
        let tera = match Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Tera parsing error(s): {}", e);
                ::std::process::exit(1);
            },
        };
        App::new()
            .app_data(web::Data::new(tera))
            .app_data(web::Data::new(pool.clone()))
            .wrap(middleware::Logger::default())
            .service(Files::new("/static", "static/"))
            .service(index)
            .service(search)
            .service(player)
            .service(about)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
