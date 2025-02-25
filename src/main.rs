use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use std::sync::Mutex;
use tera::{Context, Tera};

mod dat_decompress;
mod dat_parser;
mod pf_parser;

use dat_parser::{ArchiveId, DatFile, hex_dump};

struct AppState {
    dat_file: Mutex<Option<DatFile>>,
    tera: Tera,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let file_path = "/home/ridwan/.local/share/Steam/steamapps/common/Guild Wars 2/Gw2.dat";
    let server_address = "127.0.0.1:8080";

    // Initialize the shared state with the DAT file
    let dat_file = DatFile::load(file_path).ok();
    if dat_file.is_some() {
        println!("DAT file loaded successfully from: {}", file_path);
    } else {
        println!("Failed to load DAT file from: {}", file_path);
    }

    // Initialize Tera templates
    let tera = Tera::new("templates/**/*").expect("Error initializing Tera templates");

    let app_state = web::Data::new(AppState {
        dat_file: Mutex::new(dat_file),
        tera,
    });

    // Start the Actix Web server
    println!("Starting server at: {}\n", server_address);
    // Print each route's address and description
    println!(
        "Route: {}/ (GET) - Home page, returns the main interface of the server.",
        server_address
    );
    println!(
        "Route: {}/extract/base_id/{{index_number}} (GET) - Extracts data using the base ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/extract/file_id/{{index_number}} (GET) - Extracts data using the file ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/download/compressed/base_id/{{index_number}} (GET) - Downloads compressed data using the base ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/download/compressed/file_id/{{index_number}} (GET) - Downloads compressed data using the file ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/download/decompressed/base_id/{{index_number}} (GET) - Downloads decompressed data using the base ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/download/decompressed/file_id/{{index_number}} (GET) - Downloads decompressed data using the file ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/convert_to_image/base_id/{{index_number}} (GET) - Converts data to image using the base ID: {{index_number}}.",
        server_address
    );
    println!(
        "Route: {}/convert_to_image/file_id/{{index_number}} (GET) - Converts data to image using the file ID: {{index_number}}.",
        server_address
    );

    HttpServer::new(move || {
        let app = App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(index))
            .route(
                "/extract/base_id/{index_number}",
                web::get().to(extract_data_base_id),
            )
            .route(
                "/extract/file_id/{index_number}",
                web::get().to(extract_data_file_id),
            )
            .route(
                "/download/compressed/base_id/{index_number}",
                web::get().to(download_compressed_data_base_id),
            )
            .route(
                "/download/compressed/file_id/{index_number}",
                web::get().to(download_compressed_data_file_id),
            )
            .route(
                "/download/decompressed/base_id/{index_number}",
                web::get().to(download_decompressed_data_base_id),
            )
            .route(
                "/download/decompressed/file_id/{index_number}",
                web::get().to(download_decompressed_data_file_id),
            )
            .route(
                "/convert_to_image/base_id/{index_number}",
                web::get().to(convert_to_image_base_id),
            )
            .route(
                "/convert_to_image/file_id/{index_number}",
                web::get().to(convert_to_image_file_id),
            );

        app
    })
    .bind(server_address)?
    .run()
    .await
}

async fn index(data: web::Data<AppState>) -> impl Responder {
    let mut context = Context::new();
    context.insert("message", "Welcome to the GW2 DAT File API!");
    let rendered = data.tera.render("index.html", &context);

    match rendered {
        Ok(body) => HttpResponse::Ok().body(body),
        Err(err) => {
            eprintln!("Template error: {}", err);
            HttpResponse::InternalServerError().body("Template rendering error")
        }
    }
}
async fn extract_data_base_id(data: web::Data<AppState>, path: web::Path<u32>) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize) {
            Ok((raw_data, decompressed_data)) => {
                let hex_raw_data = hex_dump(&raw_data, 16, 16); // 16 bytes per line, 16 lines max
                let hex_decompressed_data = hex_dump(&decompressed_data, 16, 16);

                let mut context = Context::new();
                context.insert("index_number", &index_number);
                context.insert("raw_data", &hex_raw_data);
                context.insert("decompressed_data", &hex_decompressed_data);
                context.insert("raw_data_length", &raw_data.len());
                context.insert("decompressed_data_length", &decompressed_data.len());

                let rendered = data.tera.render("data_view_base_id.html", &context);

                match rendered {
                    Ok(body) => HttpResponse::Ok().body(body),
                    Err(err) => {
                        eprintln!("Template error: {}", err);
                        HttpResponse::InternalServerError().body("Template rendering error")
                    }
                }
            }
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn extract_data_file_id(data: web::Data<AppState>, path: web::Path<u32>) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::FileId, index_number as usize) {
            Ok((raw_data, decompressed_data)) => {
                let hex_raw_data = hex_dump(&raw_data, 16, 16); // 16 bytes per line, 16 lines max
                let hex_decompressed_data = hex_dump(&decompressed_data, 16, 16);

                let mut context = Context::new();
                context.insert("index_number", &index_number);
                context.insert("raw_data", &hex_raw_data);
                context.insert("decompressed_data", &hex_decompressed_data);
                context.insert("raw_data_length", &raw_data.len());
                context.insert("decompressed_data_length", &decompressed_data.len());

                let rendered = data.tera.render("data_view_file_id.html", &context);

                match rendered {
                    Ok(body) => HttpResponse::Ok().body(body),
                    Err(err) => {
                        eprintln!("Template error: {}", err);
                        HttpResponse::InternalServerError().body("Template rendering error")
                    }
                }
            }
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn download_compressed_data_base_id(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize) {
            Ok((raw_data, _)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!(
                        "attachment; filename=compressed_base_id_{}.bin",
                        index_number
                    ),
                ))
                .body(raw_data),
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn download_compressed_data_file_id(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::FileId, index_number as usize) {
            Ok((raw_data, _)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!(
                        "attachment; filename=compressed_file_id_{}.bin",
                        index_number
                    ),
                ))
                .body(raw_data),
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn download_decompressed_data_base_id(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize) {
            Ok((_, decompressed_data)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!(
                        "attachment; filename=decompressed_base_id_{}.bin",
                        index_number
                    ),
                ))
                .body(decompressed_data),
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn download_decompressed_data_file_id(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::FileId, index_number as usize) {
            Ok((_, decompressed_data)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!(
                        "attachment; filename=decompressed_file_id_{}.bin",
                        index_number
                    ),
                ))
                .body(decompressed_data),
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn convert_to_image_base_id(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize) {
            Ok((_, decompressed_data)) => {
                if let Some(image_type) = detect_image_format(&decompressed_data) {
                    HttpResponse::Ok()
                        .content_type(image_type)
                        .body(decompressed_data)
                } else {
                    HttpResponse::UnsupportedMediaType()
                        .body("Data is not a supported image format.")
                }
            }
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

async fn convert_to_image_file_id(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::FileId, index_number as usize) {
            Ok((_, decompressed_data)) => {
                if let Some(image_type) = detect_image_format(&decompressed_data) {
                    HttpResponse::Ok()
                        .content_type(image_type)
                        .body(decompressed_data)
                } else {
                    HttpResponse::UnsupportedMediaType()
                        .body("Data is not a supported image format.")
                }
            }
            Err(err) => {
                HttpResponse::InternalServerError().body(format!("Error extracting data: {}", err))
            }
        }
    } else {
        HttpResponse::InternalServerError().body("DAT file not loaded.")
    }
}

fn detect_image_format(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.starts_with(&[0x49, 0x49, 0x2A, 0x00])
        || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
    {
        Some("image/tiff")
    } else {
        None
    }
}
