use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::app::app::App;
use crate::app::business_logic::request::curl::parse_curl_request;
use crate::cli::args::ARGS;
use crate::cli::commands::import::CurlImport;
use crate::models::collection::Collection;
use crate::models::request::Request;

impl App<'_> {
    pub fn import_curl_file(&mut self, curl_import: &CurlImport) -> anyhow::Result<()> {
        let path_buf = &curl_import.import_path;
        let collection_name = &curl_import.collection_name;
        let request_name = &curl_import.request_name;
        let recursive = &curl_import.recursive;
        let max_depth = curl_import.max_depth.unwrap_or(99);
        
        println!("Parsing cURL request");

        println!("Collection name: {}", collection_name);

        let (collection_index, collection) = match self.collections
            .par_iter_mut()
            .enumerate()
            .find_any(|(_, collection)| collection.name == collection_name.as_str()) {
            Some((index, collection)) => (index, collection),
            None => {
                println!("Collection does not exist. Creating it...");

                let file_format = self.config.get_preferred_collection_file_format();

                let collection = Collection {
                    name: collection_name.clone(),
                    last_position: self.collections.len().checked_sub(1),
                    requests: vec![],
                    path: ARGS.directory.as_ref().unwrap().join(format!("{}.{}", collection_name.clone(), file_format.to_string())),
                    file_format,
                };

                self.collections.push(collection);

                (self.collections.len()-1, self.collections.last_mut().unwrap())
            }
        };

        let request_name = match request_name {
            None => path_buf.file_stem().unwrap().to_str().unwrap().to_string(),
            Some(request_name) => request_name.clone()
        };

        let requests = match path_buf.is_file() {
            true => vec![
                parse_request(path_buf, request_name)?
            ],
            false => parse_requests_recursively(path_buf, *recursive, max_depth)?,
        };

        // Add the parsed request to the collection
        collection.requests.extend(requests);

        self.save_collection_to_file(collection_index);
        
        Ok(())
    }
}

fn parse_requests_recursively(path: &PathBuf, recursive: bool, max_depth: u16) -> anyhow::Result<Vec<Arc<RwLock<Request>>>> {
    let max_depth: usize = match recursive {
        true => max_depth as usize,
        false => 1
    };

    let mut requests: Vec<Arc<RwLock<Request>>> = vec![];
    let walker = WalkDir::new(path)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok());

    for entry in walker {
        if !entry.file_type().is_file() {
            continue;
        }

        // Will use the file name as the request name
        let file_name = entry.file_name().to_str().unwrap().to_string();
        let request = parse_request(&entry.path().to_path_buf(), file_name)?;

        requests.push(request);
    }

    return Ok(requests);
}

fn parse_request(path: &PathBuf, request_name: String) -> anyhow::Result<Arc<RwLock<Request>>> {
    let curl_stringed = match fs::read_to_string(path) {
        Ok(original_curl) => original_curl,
        Err(e) => {
            return Err(anyhow::anyhow!("Could not read cURL file\n\t{}", e))
        },
    };

    println!("\tRequest name: {}", request_name);

    Ok(Arc::new(RwLock::new(parse_curl_request(&curl_stringed, request_name)?)))
}
