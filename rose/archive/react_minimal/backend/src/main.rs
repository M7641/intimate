use ntex::web;
use ntex_files as fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct User {
    id: i32,
    name: String,
    age: i32,
}

async fn users() -> impl web::Responder {
    let user_list = vec![
        User {
            id: 1,
            name: "John Doe".to_string(),
            age: 25,
        },
        User {
            id: 2,
            name: "Jane Doe".to_string(),
            age: 30,
        },
    ];
    let serialised = serde_json::to_string(&user_list).unwrap();
    web::HttpResponse::Ok().body(serialised)
}

#[ntex::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server at: http://{}:{}", "0.0.0.0", 8051);

    web::HttpServer::new(|| {
        web::App::new()
            .wrap(web::middleware::Logger::default())
            .route("/api/users", web::get().to(users))
            .service(
                fs::Files::new("/", "./dist/")
                    .index_file("index.html")
                    .show_files_listing(),
            )
    })
    .bind(("0.0.0.0", 8051))?
    .run()
    .await
}
