mod logic_helper;
mod web;

use crate::{
    logic_helper::LogicData,
    web::{AppData, VersionInfo, VERSION},
};
use actix_easy_multipart::MultipartFormConfig;
use actix_files::NamedFile;
use actix_web::{
    middleware::{Compress, Logger},
    App, HttpServer,
};
use clap::Parser;
use log::info;
use maprando::{
    customize::{mosaic::MosaicTheme, samus_sprite::SamusSpriteCategory},
    map_repository::MapRepository,
    preset::PresetData,
    seed_repository::SeedRepository,
};
use maprando_game::GameData;
use std::{path::Path, time::Instant};
use web::{about, generate, home, logic, randomize, releases, seed};

const VISUALIZER_PATH: &'static str = "../visualizer/";

#[derive(Parser)]
struct Args {
    #[arg(long)]
    seed_repository_url: String,
    #[arg(long, default_value = "https://map-rando-videos.b-cdn.net")]
    video_storage_url: String,
    #[arg(long)]
    video_storage_path: Option<String>,
    #[arg(long, action)]
    debug: bool,
    #[arg(long, action)]
    static_visualizer: bool,
    #[arg(long)]
    parallelism: Option<usize>,
    #[arg(long, action)]
    dev: bool,
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

fn load_visualizer_files() -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = vec![];
    for entry_res in std::fs::read_dir(VISUALIZER_PATH).unwrap() {
        let entry = entry_res.unwrap();
        let name = entry.file_name().to_str().unwrap().to_string();
        let data = std::fs::read(entry.path()).unwrap();
        files.push((name, data));
    }
    files
}

fn build_app_data() -> AppData {
    let start_time = Instant::now();
    let args = Args::parse();
    let sm_json_data_path = Path::new("../sm-json-data");
    let room_geometry_path = Path::new("../room_geometry.json");
    let escape_timings_path = Path::new("data/escape_timings.json");
    let start_locations_path = Path::new("data/start_locations.json");
    let hub_locations_path = Path::new("data/hub_locations.json");
    let etank_colors_path = Path::new("data/etank_colors.json");
    let reduced_flashing_path = Path::new("data/reduced_flashing.json");
    let strat_videos_path = Path::new("data/strat_videos.json");
    let vanilla_map_path = Path::new("../maps/vanilla");
    let tame_maps_path = Path::new("../maps/v113-tame");
    let wild_maps_path = Path::new("../maps/v110c-wild");
    let samus_sprites_path = Path::new("../MapRandoSprites/samus_sprites/manifest.json");
    let title_screen_path = Path::new("../TitleScreen/Images");
    let tech_path = Path::new("data/tech_data.json");
    let notable_path = Path::new("data/notable_data.json");
    let presets_path = Path::new("data/presets");
    let mosaic_themes = vec![
        ("OuterCrateria", "Outer Crateria"),
        ("InnerCrateria", "Inner Crateria"),
        ("GreenBrinstar", "Green Brinstar"),
        ("RedBrinstar", "Red Brinstar"),
        ("UpperNorfair", "Upper Norfair"),
        ("WreckedShip", "Wrecked Ship"),
        ("WestMaridia", "West Maridia"),
        ("YellowMaridia", "Yellow Maridia"),
        ("MechaTourian", "Mecha Tourian"),
        ("MetroidHabitat", "Metroid Habitat"),
    ]
    .into_iter()
    .map(|(x, y)| MosaicTheme {
        name: x.to_string(),
        display_name: y.to_string(),
    })
    .collect();

    let game_data = GameData::load(
        sm_json_data_path,
        room_geometry_path,
        escape_timings_path,
        start_locations_path,
        hub_locations_path,
        title_screen_path,
        reduced_flashing_path,
        strat_videos_path,
    )
    .unwrap();

    info!("Loading logic preset data");
    let etank_colors: Vec<Vec<String>> =
        serde_json::from_str(&std::fs::read_to_string(&etank_colors_path).unwrap()).unwrap();
    let version_info = VersionInfo {
        version: VERSION,
        dev: args.dev,
    };
    let video_storage_url = if args.video_storage_path.is_some() {
        "/videos".to_string()
    } else {
        args.video_storage_url.clone()
    };

    let preset_data = PresetData::load(tech_path, notable_path, presets_path, &game_data).unwrap();

    let logic_data = LogicData::new(&game_data, &preset_data, &version_info, &video_storage_url);
    let samus_sprite_categories: Vec<SamusSpriteCategory> =
        serde_json::from_str(&std::fs::read_to_string(&samus_sprites_path).unwrap()).unwrap();
    let app_data = AppData {
        game_data,
        preset_data,
        map_repositories: vec![
            (
                "Vanilla".to_string(),
                MapRepository::new("Vanilla", vanilla_map_path).unwrap(),
            ),
            (
                "Tame".to_string(),
                MapRepository::new("Tame", tame_maps_path).unwrap(),
            ),
            (
                "Wild".to_string(),
                MapRepository::new("Wild", wild_maps_path).unwrap(),
            ),
        ]
        .into_iter()
        .collect(),
        seed_repository: SeedRepository::new(&args.seed_repository_url).unwrap(),
        visualizer_files: load_visualizer_files(),
        video_storage_url,
        video_storage_path: args.video_storage_path.clone(),
        logic_data,
        samus_sprite_categories,
        _debug: args.debug,
        port: args.port,
        version_info: VersionInfo {
            version: VERSION,
            dev: args.dev,
        },
        static_visualizer: args.static_visualizer,
        etank_colors,
        mosaic_themes,
    };
    info!("Start-up time: {:.3}s", start_time.elapsed().as_secs_f32());
    app_data
}

pub async fn fav_icon() -> actix_web::Result<actix_files::NamedFile> {
    Ok(NamedFile::open("static/favicon.ico")?)
}

#[actix_web::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let app_data = actix_web::web::Data::new(build_app_data());

    let port = app_data.port;

    HttpServer::new(move || {
        let mut app = App::new()
            .wrap(Compress::default())
            .app_data(app_data.clone())
            .app_data(
                MultipartFormConfig::default()
                    .memory_limit(16_000_000)
                    .total_limit(16_000_000),
            )
            .wrap(Logger::default())
            .service(home::home)
            .service(releases::releases)
            .service(generate::generate)
            .service(randomize::randomize)
            .service(about::about)
            .service(seed::scope())
            .service(logic::scope())
            .service(actix_files::Files::new(
                "/static/sm-json-data",
                "../sm-json-data",
            ))
            .service(actix_files::Files::new("/static", "static"))
            .service(actix_files::Files::new("/wasm", "maprando-wasm/pkg"))
            .route("/favicon.ico", actix_web::web::get().to(fav_icon));

        if let Some(path) = &app_data.video_storage_path {
            app = app.service(actix_files::Files::new("/videos", path));
        }
        app
    })
    .bind(("0.0.0.0", port))
    .unwrap()
    .run()
    .await
    .unwrap();
}
