use std::fs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;

use chrono::{NaiveDateTime, DateTime, Utc};
use image::io::Reader as ImageReader;
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use image_builder::{Image, colors, Picture, FilterType, Text};

const IMAGE_HEIGHT: u32 = 1024;
const IMAGE_WIDTH: u32 = 2048;
const IMAGE_MAX_HEIGHT: u32 = 512;
const IMAGE_MAX_WIDTH: u32 = 384;

const SCORE_FONT_SIZE: u32 = 256;
const SCORE_FONT_SIZE_PIXELS: u32 = SCORE_FONT_SIZE / 7;
const DATE_FONT_SIZE: u32 = 64;
const SCORE_FONT: &[u8] = include_bytes!("assets/SpaceGrotesk-Bold.ttf");

lazy_static! {
    static ref TOKEN_JSON: Value = json!({
            "image": "",
            "name": "",
            "description": "",
            "external_link": null,
            "animation_url": null,
            "traits": []
        }
    );
}
const TOKEN_IMAGE_KEY: &str = "image";
const TOKEN_NAME_KEY: &str = "name";
const TOKEN_DESCRIPTION_KEY: &str = "description";


#[derive(Serialize, Deserialize, Debug, Default, Hash)]
struct Team {
    name: String,
    logo_url: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Hash)]
enum TicketStatus {
    #[serde(rename = "finished")]
    Finished {
        _0: u64,
        _1: u64,
    },
    #[serde(rename = "active")]
    #[default]
    Active,
}

#[derive(Serialize, Deserialize, Debug, Default, Hash)]
pub struct Ticket {
    id: String,
    host_team: Team,
    guest_team: Team,
    date: u64,
    status: TicketStatus
}

impl Ticket {
    pub async fn render(&self) -> String {
        let home_team_logo_file_path = download_image(&self.host_team.logo_url)
            .await;
        let guest_team_logo_file_path = download_image(&self.guest_team.logo_url)
            .await;

        let home_team_logo = ImageReader::open(&home_team_logo_file_path)
            .expect("should open a home team logo")
            .decode()
            .expect("should be valid image")
            .into_rgba8();
        let guest_team_logo = ImageReader::open(&guest_team_logo_file_path)
            .expect("should open a guest team logo")
            .decode()
            .expect("should be valid image")
            .into_rgba8();        

        let score = match &self.status {
            TicketStatus::Finished { _0, _1 } => format!("{} - {}", _0, _1),
            _ => String::from("0 - 0")
        };

        let date = NaiveDateTime::from_timestamp_opt(self.date as i64, 0)
            .map(|time| {
                let date = DateTime::<Utc>::from_utc(time, Utc);

                date.format("%Y-%m-%d %H:%M").to_string()
            })
            .expect("should be valid timestamp");

        let mut image = Image::new(IMAGE_WIDTH, IMAGE_HEIGHT, colors::WHITE);

        image.add_custom_font("score_font", SCORE_FONT.to_vec());

        image.add_picture(
            Picture::new(&home_team_logo_file_path)
                .position(
                    IMAGE_WIDTH / 12,
                    (IMAGE_HEIGHT / 2) - (home_team_logo.height() / 2)
                )
                .resize(
                    if home_team_logo.width() > IMAGE_MAX_WIDTH {
                        IMAGE_MAX_WIDTH
                    } else {
                        home_team_logo.width()
                    },
                    if home_team_logo.height() > IMAGE_MAX_HEIGHT {
                        IMAGE_MAX_HEIGHT
                    } else {
                        home_team_logo.height()
                    },
                    FilterType::Lanczos3
                )
        );

        image.add_text(
            Text::new(&score)
                .font("score_font")
                .size(SCORE_FONT_SIZE)
                .color(colors::BLUE)
                .position(
                    IMAGE_WIDTH / 2 - (score.len() as u32 * SCORE_FONT_SIZE_PIXELS),
                    IMAGE_HEIGHT / 2 - (SCORE_FONT_SIZE),
                )
        );

        image.add_text(
            Text::new(&date)
                .color(colors::BLACK)
                .size(DATE_FONT_SIZE)
                .position(
                    IMAGE_WIDTH / 2 - 200,
                    IMAGE_HEIGHT / 2,
                )
        );

        image.add_picture(
            Picture::new(&guest_team_logo_file_path)
                .position(
                    IMAGE_WIDTH - (IMAGE_WIDTH / 12) - guest_team_logo.width(),
                    (IMAGE_HEIGHT / 2) - (guest_team_logo.height() / 2)
                )
                .resize(
                    if guest_team_logo.width() > IMAGE_MAX_WIDTH {
                        IMAGE_MAX_WIDTH
                    } else {
                        guest_team_logo.width()
                    },
                    if guest_team_logo.height() > IMAGE_MAX_HEIGHT {
                        IMAGE_MAX_HEIGHT
                    } else {
                        guest_team_logo.height()
                    },
                    FilterType::Lanczos3
                )
        );

        let image_name = format!("{}.png", calculate_hash(&self));

        image.save(&image_name);

        fs::remove_file(&home_team_logo_file_path)
            .expect("should be able to remove a file");
        
        fs::remove_file(&guest_team_logo_file_path)
            .expect("should be able to remove a file");

        image_name
    }

    pub fn make_token(&self, image_uri: &str) -> String {
        let score = match &self.status {
            TicketStatus::Finished { _0, _1 } => format!("{} - {}", _0, _1),
            _ => String::from("0 - 0")
        };

        let date = NaiveDateTime::from_timestamp_opt(self.date as i64, 0)
            .map(|time| {
                let date = DateTime::<Utc>::from_utc(time, Utc);

                date.format("%Y-%m-%d %H:%M").to_string()
            })
            .expect("should be valid timestamp");
        
        let description = format!(
            "The match between {} and {} took place on {}. The final score was {}",
            self.host_team.name,
            self.guest_team.name,
            date,
            score
        );

        let token_name = format!("{} vs {} ticket", self.host_team.name, self.guest_team.name);

        let mut raw_token = TOKEN_JSON.clone();

        raw_token[TOKEN_IMAGE_KEY] = Value::String(image_uri.to_string());
        raw_token[TOKEN_NAME_KEY] = Value::String(token_name);
        raw_token[TOKEN_DESCRIPTION_KEY] = Value::String(description);

        let token = serde_json::to_vec(&raw_token)
            .expect("should be able to serialize the token");

        let file_name = format!("{}.json", calculate_hash(&token));

        let mut file = fs::File::create(&file_name)
            .expect("should be able to create a file");

        file.write_all(&token)
            .expect("should be able to write the token");

        file_name
    }
}

async fn download_image(url: &str) -> String {
    let response = reqwest::get(url)
        .await
        .expect("should download an image");

    let body = response
        .bytes()
        .await
        .expect("should get a body");

    let tmp_file_name = format!("{}.png", calculate_hash(&body));

    fs::write(&tmp_file_name, body)
        .expect("should be able to write to tmp file");

    tmp_file_name
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();

    t.hash(&mut s);

    s.finish()
}

