mod database;
mod model;
mod schema;
use crate::schema::votes::vote_value;
use actix_files::Files;
use actix_web::middleware::Logger;
use actix_web::{post, web, App, HttpResponse, HttpServer};
use database::setup;
use diesel::dsl::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use env_logger::Env;
use handlebars::Handlebars;
use model::NewVote;
use r2d2::Pool;
use schema::votes::dsl::votes;
use serde::Deserialize;
use serde_json::json;
use std::fmt;
use std::sync::Mutex;

#[derive(Debug, Deserialize)]
enum VoteValue {
    Dogs,
    Cats,
    Reset,
}

impl fmt::Display for VoteValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VoteValue::Cats => write!(f, "Cats"),
            VoteValue::Dogs => write!(f, "Dogs"),
            VoteValue::Reset => write!(f, "Reset"),
        }
    }
}

#[derive(Deserialize)]
struct FormData {
    vote: VoteValue,
}

struct AppStateVoteCounter {
    dog_counter: Mutex<i64>, // <- Mutex is necessary to mutate safely across threads
    cat_counter: Mutex<i64>,
}

/// extract form data using serde
/// this handler gets called only if the content type is *x-www-form-urlencoded*
/// and the content of the request could be deserialized to a `FormData` struct
#[post("/")]
async fn submit(
    form: web::Form<FormData>,
    data: web::Data<AppStateVoteCounter>,
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    hb: web::Data<Handlebars<'_>>,
) -> HttpResponse {
    let mut dog_counter = data.dog_counter.lock().unwrap(); // <- get counter's MutexGuard
    let mut cat_counter = data.cat_counter.lock().unwrap();

    match &form.vote {
        VoteValue::Dogs => *dog_counter += 1, // <- access counter inside MutexGuard
        VoteValue::Cats => *cat_counter += 1,
        VoteValue::Reset => {
            *dog_counter = 0;
            *cat_counter = 0;
        }
    }

    let data = json!({
        "title": "Azure Voting App",
        "button1": "Dogs",
        "button2": "Cats",
        "value1": dog_counter.to_string(),
        "value2": cat_counter.to_string()
    });

    let body = hb.render("index", &data).unwrap();

    // if the vote value is not reset then save the
    if !matches!(&form.vote, VoteValue::Reset) {
        let vote_data = NewVote {
            vote_value: form.vote.to_string(),
        };

        let mut connection = pool.get().unwrap();
        let _vote_data = web::block(move || {
            diesel::insert_into(votes)
                .values(vote_data)
                .execute(&mut connection)
        })
        .await;
    } else {
        let mut connection = pool.get().unwrap();
        let _vote_data = web::block(move || {
            diesel::delete(votes).execute(&mut connection);
        })
        .await;
    }

    HttpResponse::Ok().body(body)
}

async fn index(
    data: web::Data<AppStateVoteCounter>,
    hb: web::Data<Handlebars<'_>>,
) -> HttpResponse {
    let dog_counter = data.dog_counter.lock().unwrap(); // <- get counter's MutexGuard
    let cat_counter = data.cat_counter.lock().unwrap();

    let data = json!({
        "title": "Azure Voting App",
        "button1": "Dogs",
        "button2": "Cats",
        "value1": dog_counter.to_string(),
        "value2": cat_counter.to_string()
    });
    let body = hb.render("index", &data).unwrap();
    HttpResponse::Ok().body(body)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = setup();

    // Default logging format is:
    // %a %t "%r" %s %b "%{Referer}i" "%{User-Agent}i" %T
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let mut connection = pool.get().unwrap();

    // Load up the dog votes
    let dog_query = votes.filter(vote_value.eq("Dogs"));
    let dog_result = dog_query.select(count(vote_value)).first(&mut connection);
    let dog_count = dog_result.unwrap_or(0);

    // Load up the cat votes
    let cat_query = votes.filter(vote_value.eq("Cats"));
    let cat_result = cat_query.select(count(vote_value)).first(&mut connection);
    let cat_count = cat_result.unwrap_or(0);

    // Note: web::Data created _outside_ HttpServer::new closure
    let vote_counter = web::Data::new(AppStateVoteCounter {
        dog_counter: Mutex::new(dog_count),
        cat_counter: Mutex::new(cat_count),
    });

    let mut handlebars = Handlebars::new();
    handlebars
        .register_templates_directory(".html", "./static/")
        .unwrap();
    let handlebars_ref = web::Data::new(handlebars);

    println!("Listening on port 8080");
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            // .wrap(Logger::new("%a %{User-Agent}i")) // <- optionally create your own format
            .app_data(vote_counter.clone()) // <- register the created data
            .app_data(handlebars_ref.clone())
            .data(pool.clone())
            .service(Files::new("/static", "static").show_files_listing())
            .route("/", web::get().to(index))
            .service(submit)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
