#![feature(seek_stream_len)]

use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use std::io;
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
async fn main() -> io::Result<()> {
    let file_path = "Local.dat";
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
    println!("Starting server at: {}", server_address);
    HttpServer::new(move || {
        App::new()
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
                "/download/base_id/{index_number}",
                web::get().to(download_data_base_id),
            )
            .route(
                "/download/file_id/{index_number}",
                web::get().to(download_data_file_id),
            )
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

                let rendered = data.tera.render("data_view.html", &context);

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

                let rendered = data.tera.render("data_view.html", &context);

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

async fn download_data_base_id(data: web::Data<AppState>, path: web::Path<u32>) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize) {
            Ok((raw_data, _)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=base_id_{}.bin", index_number),
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

async fn download_data_file_id(data: web::Data<AppState>, path: web::Path<u32>) -> impl Responder {
    let index_number = path.into_inner();

    let mut dat_file = data.dat_file.lock().unwrap();
    if let Some(dat_file) = dat_file.as_mut() {
        match dat_file.extract_mft_data(ArchiveId::FileId, index_number as usize) {
            Ok((raw_data, _)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=file_id_{}.bin", index_number),
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
