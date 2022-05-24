use actix_web::{error, web, Error};
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type Connection = r2d2::PooledConnection<SqliteConnectionManager>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: i32,
    tag: String,
    prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerIntMetric {
    metric: i32,
    id: i32,
    tag: String,
    prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerFloatMetric {
    metric: f32,
    id: i32,
    tag: String,
    prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Standing {
    placement: i32,
    tournament_name: String,
    player_id: i32,
    player_tag: String,
    player_prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Set {
    id: i32,
    event: i32,
    event_name: String,
    tournament_name: String,
    round: String,
    winner: i32,
    winner_score: i32,
    winner_tag: String,
    winner_prefix: String,
    loser: i32,
    loser_score: i32,
    loser_tag: String,
    loser_prefix: String,
}

pub async fn players_by_tag(pool: &Pool, tag: String) -> Result<Vec<Player>, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;

    web::block(move || get_players_by_tag(conn, &tag))
        .await?
        .map_err(error::ErrorInternalServerError)
}

fn get_players_by_tag(conn: Connection, tag: &String) -> Result<Vec<Player>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "select * 
        from 
            player 
        where 
            lower(player_tag) like lower('%{}%') 
            and exclude==0",
        tag
    ))?;

    stmt.query_map([], |row| {
        Ok(Player {
            id: row.get(0)?,
            tag: row.get(1)?,
            prefix: row.get(2)?,
        })
    })
    .and_then(Iterator::collect)
}

pub async fn player_by_id(pool: &Pool, id: i32) -> Result<Player, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;
    web::block(move || get_player_by_id(conn, &id))
        .await?
        .map_err(error::ErrorInternalServerError)
}

fn get_player_by_id(conn: Connection, id: &i32) -> Result<Player, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("select * from player where player_id = {}", id))?;

    stmt.query_row([], |row| {
        Ok(Player {
            id: row.get(0)?,
            tag: row.get(1)?,
            prefix: row.get(2)?,
        })
    })
}

pub async fn most_matches_played(pool: &Pool) -> Result<Vec<PlayerIntMetric>, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;

    web::block(move || get_most_matches_played(conn))
        .await?
        .map_err(error::ErrorInternalServerError)
}

// get the top 20 amount of matches played
fn get_most_matches_played(conn: Connection) -> Result<Vec<PlayerIntMetric>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "select
            sum( sets.set_winner_score + sets.set_loser_score) as total, 
            player.player_id, 
            player.player_tag, 
            player.player_prefix 
        from
            sets
        inner join player on sets.set_winner=player.player_id
        or sets.set_loser=player.player_id
        group by player.player_id
        order by total desc
        limit 20",
    )?;

    stmt.query_map([], |row| {
        Ok(PlayerIntMetric {
            metric: row.get(0)?,
            id: row.get(1)?,
            tag: row.get(2)?,
            prefix: row.get(3)?,
        })
    })
    .and_then(Iterator::collect)
}

pub async fn player_metrics(pool: &Pool, id: i32) -> Result<Vec<i32>, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;
    web::block(move || get_player_metrics(conn, &id))
        .await?
        .map_err(error::ErrorInternalServerError)
}

// get the matches won, total matches, sets won, & total sets
fn get_player_metrics(conn: Connection, id: &i32) -> Result<Vec<i32>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "
        select
            count(*)
        from
            sets
        where
            set_winner = {}
        ",
        id
    ))?;

    let set_wins: i32 = stmt.query_row([], |row| Ok(row.get(0)?)).unwrap_or(0);

    let mut stmt = conn.prepare(&format!(
        "
         select
            count(*)
        from
            sets
        where
            set_winner = {}
        or
            set_loser = {}
        ",
        id, id
    ))?;

    let set_total: i32 = stmt.query_row([], |row| Ok(row.get(0)?)).unwrap_or(0);

    let mut stmt = conn.prepare(&format!(
        "
        select
            sum(case
                when set_winner = {} then set_winner_score
                else set_loser_score
            end)
        from
            sets
        where
            set_winner = {}
        or
            set_loser = {}
        ",
        id, id, id
    ))?;

    let match_wins: i32 = stmt.query_row([], |row| Ok(row.get(0)?)).unwrap_or(0);

    let mut stmt = conn.prepare(&format!(
        "
        select
            sum( set_winner_score + set_loser_score)
        from
            sets
        where
            set_winner = {}
        or
            set_loser = {}
        ",
        id, id
    ))?;

    let match_total: i32 = stmt.query_row([], |row| Ok(row.get(0)?)).unwrap_or(0);

    Ok(vec![set_wins, set_total, match_wins, match_total])
}

pub async fn recent_winners(pool: &Pool) -> Result<Vec<Standing>, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;
    web::block(move || get_recent_winners(conn))
        .await?
        .map_err(error::ErrorInternalServerError)
}

// get 15 most recent top-3 placements
fn get_recent_winners(conn: Connection) -> Result<Vec<Standing>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "
        select
            standing.standing_placement as place,
            event.tournament_name,
            standing.standing_player,
            player.player_tag,
            player.player_prefix
        from
            standing
        inner join event on
            standing.standing_event=event.event_id
            and (place == 1 or place == 2 or place == 3)
        inner join player on
            standing.standing_player=player.player_id
        order by event.event_start desc
        limit 15
        ",
    )?;

    stmt.query_map([], |row| {
        Ok(Standing {
            placement: row.get(0)?,
            tournament_name: row.get(1)?,
            player_id: row.get(2)?,
            player_tag: row.get(3)?,
            player_prefix: row.get(4)?,
        })
    })
    .and_then(Iterator::collect)
}

pub async fn player_standings(pool: &Pool, id: i32) -> Result<Vec<Standing>, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;
    web::block(move || get_player_standings(conn, &id))
        .await?
        .map_err(error::ErrorInternalServerError)
}

// get 20 most recent standings
fn get_player_standings(conn: Connection, id: &i32) -> Result<Vec<Standing>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "
        select
            standing.standing_placement,
            event.tournament_name
        from
            standing
        inner join event on
            standing.standing_event=event.event_id
            and standing.standing_player={}
        order by event.event_start desc
        limit 20
        ",
        id
    ))?;

    stmt.query_map([], |row| {
        Ok(Standing {
            placement: row.get(0)?,
            tournament_name: row.get(1)?,
            player_id: 0,
            player_tag: "".to_string(),
            player_prefix: "".to_string(),
        })
    })
    .and_then(Iterator::collect)
}

pub async fn player_sets(pool: &Pool, id: i32) -> Result<Vec<Set>, Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;
    web::block(move || get_player_sets(conn, &id))
        .await?
        .map_err(error::ErrorInternalServerError)
}

// get 20 most recent standings
fn get_player_sets(conn: Connection, id: &i32) -> Result<Vec<Set>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "
        select
            sets.set_id,
            event.event_id,
            event.event_name,
            event.tournament_name,
            sets.set_round,
            sets.set_winner,
            sets.set_winner_score,
            A.player_tag,
            A.player_prefix,
            sets.set_loser,
            sets.set_loser_score,
            B.player_tag,
            B.player_prefix
        from
            sets
        inner join event on
            sets.set_event=event.event_id
            and (set_winner={} or set_loser={})
        inner join player A on
            sets.set_winner=A.player_id
        inner join player B on
            sets.set_loser=B.player_id
        order by sets.set_id desc
        ",
        id, id
    ))?;

    stmt.query_map([], |row| {
        Ok(Set {
            id: row.get(0)?,
            event: row.get(1)?,
            event_name: row.get(2)?,
            tournament_name: row.get(3)?,
            round: row.get(4)?,
            winner: row.get(5)?,
            winner_score: row.get(6)?,
            winner_tag: row.get(7)?,
            winner_prefix: row.get(8)?,
            loser: row.get(9)?,
            loser_score: row.get(10)?,
            loser_tag: row.get(11)?,
            loser_prefix: row.get(12)?,
        })
    })
    .and_then(Iterator::collect)
}
