use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, post};
use handlebars::Handlebars;
use serde_json::json;
use std::sync::Mutex;
use serde::Deserialize;
use actix_web::middleware::Logger;
use env_logger::Env;

#[derive(Deserialize)]
enum VoteValue {
    Dogs,
    Cats,
    Reset
}

#[derive(Deserialize)]
struct FormData {
    vote: VoteValue,
}

struct AppStateVoteCounter {
    dog_counter: Mutex<i32>, // <- Mutex is necessary to mutate safely across threads
    cat_counter: Mutex<i32>,
}

/// extract form data using serde
/// this handler gets called only if the content type is *x-www-form-urlencoded*
/// and the content of the request could be deserialized to a `FormData` struct
#[post("/")]
async fn submit(form: web::Form<FormData>, data: web::Data<AppStateVoteCounter>, hb: web::Data<Handlebars<'_>>) -> HttpResponse {
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
    HttpResponse::Ok().body(body)
}

async fn index(data: web::Data<AppStateVoteCounter>, hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    let dog_counter = data.dog_counter.lock().unwrap(); // <- get dog_counter's MutexGuard
    let cat_counter = data.cat_counter.lock().unwrap(); // <- get cat_counter's MutexGuard
    
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
    // Default logging format is:
    // %a %t "%r" %s %b "%{Referer}i" "%{User-Agent}i" %T
    env_logger::init_from_env(Env::default().default_filter_or("info"));    

    // Note: web::Data created _outside_ HttpServer::new closure
    let vote_counter = web::Data::new(AppStateVoteCounter {
        dog_counter: Mutex::new(0),
        cat_counter: Mutex::new(0),
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
            .service(Files::new("/static", "static").show_files_listing())
            .route("/", web::get().to(index))
            .service(submit)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
