mod ticket;

use std::env;
use std::mem;
use std::fs;

use actix_web::http::header::ContentType;
use serde::{Serialize, Deserialize};
use actix_web::{HttpServer, App, Responder, post, HttpResponse, web, };
use ipfs_api_backend_hyper::{IpfsClient, TryFromUri, IpfsApi};

use ticket::Ticket;

const PORT_ENV_VAR: &str = "PORT";
const IPFS_USERNAME_ENV_VAR: &str = "IPFS_USERNAME";
const IPFS_PASSWORD_ENV_VAR: &str = "IPFS_PASSWORD";
const IPFS_PROVIDER_HOST: &str = "https://ipfs.infura.io:5001";

#[derive(Serialize, Deserialize)]
enum IpfsResponse {
    #[serde(rename = "response")]
    Response {
        token_uri: String
    },
    #[serde(rename = "error")]
    Error {
        msg: String
    }
}

#[post("/upload_match")]
async fn upload_match(data: web::Data<AppData>, ticket: web::Json<Ticket>) -> impl Responder {    
    let image_name = ticket
        .render()
        .await;

    let image = fs::File::open(&image_name)
        .expect("image should be present");

    let result = data.ipfs_client.add(image)
        .await
        .expect("should be able to deploy to aws");

    fs::remove_file(&image_name)
        .expect("should be able to remove a file");

    let token_name = ticket.make_token(&format!("https://ipfs.io/ipfs/{}", result.hash));

    let token = fs::File::open(&token_name)
        .expect("token should be present");

    let result = data.ipfs_client.add(token)
        .await
        .expect("should be able to deploy to aws");

    fs::remove_file(&token_name)
        .expect("should be able to remove a file");

    let response = IpfsResponse::Response {
        token_uri: format!("https://ipfs.io/ipfs/{}", result.hash)
    };

    let response_body = serde_json::to_string(&response)
        .expect("should be able to serialize the response");

    HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(response_body)
}

struct AppData {
    ipfs_client: IpfsClient,
}

impl AppData {
    fn new() -> Self {
        Self {
            ipfs_client: get_ipfs_client(),
        }
    }
}

impl Clone for AppData {
    fn clone(&self) -> Self {
        Self {
            ipfs_client: unsafe { mem::transmute_copy(&self.ipfs_client) },
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let server_port = env::var(PORT_ENV_VAR)
        .map(|env| env.parse::<u16>())
        .expect(&format!("{} enviroment variable should present", PORT_ENV_VAR))
        .expect("invalid port");

    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(AppData::new()))
            .service(upload_match)
    })
    .bind(("0.0.0.0", server_port))?
    .run()
    .await
}

fn get_ipfs_client() -> IpfsClient {
    let username = env::var(IPFS_USERNAME_ENV_VAR)
        .expect(&format!("{} enviroment variable should present", IPFS_USERNAME_ENV_VAR));
    let password = env::var(IPFS_PASSWORD_ENV_VAR)
        .expect(&format!("{} enviroment variable should present", IPFS_PASSWORD_ENV_VAR));

    IpfsClient::from_str(IPFS_PROVIDER_HOST)
        .map(|client| client.with_credentials(username, password))
        .expect("backend should connect")
}
