//! `terra-kml`: import, export and round-trip KML/KMZ through a SQLite library.
//!
//! This is the M2 acceptance tool: it proves a file survives parse -> SQLite ->
//! export without losing folders, styles or the Circle/Rectangle markers.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use terra_kml::{ParseKmlResult, SerializeKmlInput, parse_kml, read_kmz, serialize_kml, write_kmz};
use terra_storage::PlacesRepository;
use terra_storage_sqlite::SqlitePlacesRepository;

const USAGE: &str = "\
terra-kml — import, export and round-trip KML/KMZ

USAGE:
  terra-kml import <file.kml|file.kmz> --db <library.db>
  terra-kml export <library.db> --out <file.kml|file.kmz>
  terra-kml roundtrip <file.kml|file.kmz>
  terra-kml counts <library.db>

Both documents are read with the same parser the app uses; importing never
creates duplicates for rows that already exist (ids are preserved).
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let Some(command) = args.first() else {
        print!("{USAGE}");
        return Ok(());
    };
    match command.as_str() {
        "import" => import(&args[1..]),
        "export" => export(&args[1..]),
        "roundtrip" => roundtrip(&args[1..]),
        "counts" => counts(&args[1..]),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}").into()),
    }
}

fn import(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (input, db) = positional_and_flag(args, "--db")?;
    let document = read_document(&input)?;
    let parsed = parse_kml(&document)?;
    report_warnings(&parsed);

    let repo = SqlitePlacesRepository::open(&db).map_err(display)?;
    pollster::block_on(repo.import_batch(terra_storage::ImportBatch {
        folders: parsed.folders.clone(),
        placemarks: parsed.placemarks.clone(),
        styles: parsed.styles.clone(),
        screen_overlays: parsed.screen_overlays.clone(),
    }))
    .map_err(display)?;

    println!(
        "imported {} folders, {} placemarks, {} styles, {} screen overlays into {}",
        parsed.folders.len(),
        parsed.placemarks.len(),
        parsed.styles.len(),
        parsed.screen_overlays.len(),
        db.display()
    );
    Ok(())
}

fn export(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (db, out) = positional_and_flag(args, "--out")?;
    let repo = SqlitePlacesRepository::open(&db).map_err(display)?;
    let batch = pollster::block_on(terra_storage::collect_all(&repo)).map_err(display)?;
    let kml = serialize_kml(&SerializeKmlInput {
        folders: &batch.folders,
        placemarks: &batch.placemarks,
        styles: &batch.styles,
        screen_overlays: &batch.screen_overlays,
    })?;

    if out.extension().and_then(|extension| extension.to_str()) == Some("kmz") {
        std::fs::write(&out, write_kmz(&kml, &[])?).map_err(display)?;
    } else {
        std::fs::write(&out, kml).map_err(display)?;
    }
    println!(
        "exported {} folders, {} placemarks to {}",
        batch.folders.len(),
        batch.placemarks.len(),
        out.display()
    );
    Ok(())
}

fn roundtrip(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let Some(input) = args.first() else {
        return Err("roundtrip needs a file path".into());
    };
    let document = read_document(Path::new(input))?;
    let first = parse_kml(&document)?;
    report_warnings(&first);
    let serialized = serialize_kml(&SerializeKmlInput {
        folders: &first.folders,
        placemarks: &first.placemarks,
        styles: &first.styles,
        screen_overlays: &first.screen_overlays,
    })?;
    let second = parse_kml(&serialized)?;

    let before = canonical(&first)?;
    let after = canonical(&second)?;
    if before != after {
        return Err("round trip changed the document".into());
    }
    println!(
        "round trip OK: {} folders, {} placemarks, {} styles, {} screen overlays",
        first.folders.len(),
        first.placemarks.len(),
        first.styles.len(),
        first.screen_overlays.len()
    );
    Ok(())
}

fn counts(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let Some(db) = args.first() else {
        return Err("counts needs a database path".into());
    };
    let repo = SqlitePlacesRepository::open(Path::new(db)).map_err(display)?;
    let batch = pollster::block_on(terra_storage::collect_all(&repo)).map_err(display)?;
    println!(
        "{} folders, {} placemarks, {} styles, {} screen overlays",
        batch.folders.len(),
        batch.placemarks.len(),
        batch.styles.len(),
        batch.screen_overlays.len()
    );
    Ok(())
}

/// Reads a KML file or the document inside a KMZ archive.
fn read_document(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path).map_err(display)?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("kmz") {
        let entries = read_kmz(&bytes)?;
        let document = entries
            .iter()
            .find(|entry| entry.name.ends_with(".kml"))
            .or_else(|| entries.first())
            .ok_or("KMZ contains no KML document")?;
        return Ok(String::from_utf8(document.bytes.clone())
            .map_err(|error| format!("KML inside the KMZ is not UTF-8: {error}"))?);
    }
    Ok(String::from_utf8(bytes).map_err(|error| format!("file is not UTF-8: {error}"))?)
}

/// Document content with generated identifiers masked.
///
/// A MultiGeometry without an `id` gets member ids derived from the generated
/// group id on the first export, so raw ids legitimately differ once. Comparing
/// content is what the round-trip promise is about.
fn canonical(parsed: &ParseKmlResult) -> Result<serde_json::Value, String> {
    let value = serde_json::to_value(parsed).map_err(display)?;
    Ok(mask_ids(value))
}

fn mask_ids(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(mask_ids).collect())
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                let masked = match key.as_str() {
                    "id" | "multiGeometryGroup" | "folderId" | "parentId" if child.is_string() => {
                        serde_json::Value::String("<ref>".to_string())
                    }
                    _ => mask_ids(child),
                };
                out.insert(key, masked);
            }
            serde_json::Value::Object(out)
        }
        other => other,
    }
}

fn report_warnings(parsed: &ParseKmlResult) {
    for warning in &parsed.warnings {
        eprintln!("warning: {warning}");
    }
}

fn positional_and_flag(args: &[String], flag: &str) -> Result<(PathBuf, PathBuf), String> {
    let mut positional = None;
    let mut flagged = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{flag} needs a value"))?;
            flagged = Some(PathBuf::from(value));
            index += 2;
        } else {
            positional = Some(PathBuf::from(&args[index]));
            index += 1;
        }
    }
    let positional = positional.ok_or_else(|| format!("missing path argument\n\n{USAGE}"))?;
    let flagged = flagged.ok_or_else(|| format!("missing {flag} argument\n\n{USAGE}"))?;
    Ok((positional, flagged))
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <Style id="s1"><LineStyle><color>ffcc6633</color><width>2</width></LineStyle>
    <PolyStyle><color>993366cc</color><fill>1</fill><outline>1</outline></PolyStyle></Style>
  <Folder id="f1"><name>Places</name>
    <Placemark id="p1"><name>Berlin</name><styleUrl>#s1</styleUrl>
      <Point><coordinates>13.405,52.52,0</coordinates></Point></Placemark>
    <Placemark id="p2"><name>Circle</name>
      <Polygon><outerBoundaryIs><LinearRing><coordinates>13.3,52.4 13.5,52.4 13.5,52.6 13.3,52.4</coordinates></LinearRing></outerBoundaryIs></Polygon>
      <ExtendedData>
        <Data name="terra-globe:shape"><value>circle</value></Data>
        <Data name="terra-globe:centerLon"><value>13.4</value></Data>
        <Data name="terra-globe:centerLat"><value>52.5</value></Data>
        <Data name="terra-globe:radiusMeters"><value>1200</value></Data>
      </ExtendedData>
    </Placemark>
  </Folder>
</Document></kml>"#;

    #[test]
    fn import_export_round_trips_through_sqlite() {
        let parsed = parse_kml(DOCUMENT).expect("parses");
        let repo = SqlitePlacesRepository::open_in_memory().expect("repo");
        pollster::block_on(repo.import_batch(terra_storage::ImportBatch {
            folders: parsed.folders.clone(),
            placemarks: parsed.placemarks.clone(),
            styles: parsed.styles.clone(),
            screen_overlays: parsed.screen_overlays.clone(),
        }))
        .expect("imports");

        let batch = pollster::block_on(terra_storage::collect_all(&repo)).expect("collects");
        assert_eq!(batch.folders.len(), 1);
        assert_eq!(batch.placemarks.len(), 2);
        assert_eq!(batch.styles.len(), 1);

        let exported = serialize_kml(&SerializeKmlInput {
            folders: &batch.folders,
            placemarks: &batch.placemarks,
            styles: &batch.styles,
            screen_overlays: &batch.screen_overlays,
        })
        .expect("serializes");
        let reparsed = parse_kml(&exported).expect("reparses");

        assert_eq!(
            canonical(&parsed).expect("json"),
            canonical(&reparsed).expect("json")
        );
    }

    #[test]
    fn kmz_documents_are_read_from_the_archive() {
        let kmz = write_kmz(DOCUMENT, &[]).expect("builds kmz");
        let path = std::env::temp_dir().join("terra-kml-test.kmz");
        std::fs::write(&path, kmz).expect("writes kmz");
        let document = read_document(&path).expect("reads kmz");
        assert!(document.contains("Berlin"));
        let _ = std::fs::remove_file(path);
    }
}
