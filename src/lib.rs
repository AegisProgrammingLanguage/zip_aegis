use aegis_core::{Value, NativeFn};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write, Read};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

pub fn register(map: &mut HashMap<String, NativeFn>) {
    map.insert("zip_extract".to_string(), zip_extract);
    map.insert("zip_compress".to_string(), zip_compress);
}

// --- EXTRACTION ---

fn zip_extract(args: Vec<Value>) -> Result<Value, String> {
    if args.len() < 2 { return Err("Args: zip_path, dest_dir".into()); }
    
    let src_path = args[0].as_str()?;
    let dest_dir = args[1].as_str()?;

    let file = fs::File::open(&src_path).map_err(|e| format!("Open failed: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        
        // SÉCURITÉ : "Zip Slip" protection
        // enclosed_name() renvoie None si le chemin tente de sortir du dossier (ex: ../../etc/passwd)
        let outpath = match file.enclosed_name() {
            Some(path) => Path::new(&dest_dir).join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            // C'est un dossier
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            // C'est un fichier
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).map_err(|e| e.to_string())?;
                }
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| e.to_string())?;
            io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    Ok(Value::Boolean(true))
}

// --- COMPRESSION ---

fn zip_compress(args: Vec<Value>) -> Result<Value, String> {
    if args.len() < 2 { return Err("Args: source_dir, zip_path".into()); }

    let src_dir = args[0].as_str()?;
    let dst_path = args[1].as_str()?;

    let file = fs::File::create(&dst_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    let walk_dir = WalkDir::new(&src_dir);
    let src_path = Path::new(&src_dir);

    for entry in walk_dir.into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // On calcule le nom relatif pour le ZIP (ex: "src/main.aeg")
        let name = path.strip_prefix(src_path)
            .map_err(|_| "Path prefix error")?
            .to_str()
            .ok_or("Invalid UTF-8 path")?;

        if path.is_file() {
            // On ajoute le fichier
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            
            let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            zip.write_all(&buffer).map_err(|e| e.to_string())?;
        } else if !name.is_empty() {
            // On ajoute le dossier (sauf si c'est la racine vide)
            zip.add_directory(name, options).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;

    Ok(Value::Boolean(true))
}
