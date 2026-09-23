use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Serialize;

#[derive(Debug, Clone)]
pub enum Format {
    Cbor,
    Json,
    Toon,
}

impl Format {
    pub const fn all() -> &'static [&'static str] {
        &["cbor", "json", "toon"]
    }

    pub fn dump_formats<T: Serialize>(
        &self,
        space: T,
        path: PathBuf,
        output_path: Option<&PathBuf>,
        pretty: bool,
    ) {
        if let Some(output_path) = output_path {
            match self {
                Self::Cbor => Cbor::with_writer(space, path, output_path),
                Self::Json => Json::with_pretty_writer(space, path, output_path, pretty),
                Self::Toon => Toon::with_writer(space, path, output_path),
            }
        } else {
            match self {
                Self::Json => Json::write_on_stdout_pretty(space, pretty),
                Self::Toon => Toon::write_on_stdout(space),
                Self::Cbor => panic!("Cbor format cannot be printed to stdout"),
            }
        }
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(format: &str) -> Result<Self, Self::Err> {
        match format {
            "cbor" => Ok(Self::Cbor),
            "json" => Ok(Self::Json),
            "toon" => Ok(Self::Toon),
            format => Err(format!("{format:?} is not a supported format")),
        }
    }
}

#[inline(always)]
fn print_on_stdout(content: String) {
    writeln!(std::io::stdout().lock(), "{content}").unwrap();
}

trait WriteOnStdout {
    #[inline(always)]
    fn write_on_stdout<T: Serialize>(content: T) {
        print_on_stdout(Self::format(content));
    }

    fn format<T: Serialize>(content: T) -> String;
}

trait WritePrettyOnStdout: WriteOnStdout {
    fn write_on_stdout_pretty<T: Serialize>(content: T, pretty: bool) {
        print_on_stdout(if pretty {
            Self::format_pretty(content)
        } else {
            Self::format(content)
        });
    }
    fn format_pretty<T: Serialize>(content: T) -> String;
}

fn handle_path(path: PathBuf, output_path: &Path, extension: &str) -> PathBuf {
    // Remove root /
    let path = path.as_path().strip_prefix("/").unwrap_or(path.as_path());

    // Remove root ./
    let path = path.strip_prefix("./").unwrap_or(path);

    // Replace .. with . to keep files inside the output folder
    let cleaned_path: Vec<&str> = path
        .iter()
        .map(|os_str| {
            let s_str = os_str.to_str().unwrap();
            if s_str == ".." { "." } else { s_str }
        })
        .collect();

    // Create the filename
    let filename = cleaned_path.join("/") + extension;

    // Build the file path
    output_path.join(filename)
}

trait WriteFile {
    const EXTENSION: &'static str;

    fn open_file(path: PathBuf, output_path: &Path) -> File {
        // Handle output path
        let format_path = handle_path(path, output_path, Self::EXTENSION);

        // Create directories
        create_dir_all(format_path.parent().unwrap()).unwrap();

        File::create(format_path).unwrap()
    }

    fn with_writer<T: Serialize>(content: T, path: PathBuf, output_path: &Path);
}

trait WritePrettyFile: WriteFile {
    fn with_pretty_writer<T: Serialize>(
        content: T,
        path: PathBuf,
        output_path: &Path,
        pretty: bool,
    );
}

struct Json;

impl WriteOnStdout for Json {
    fn format<T: Serialize>(content: T) -> String {
        serde_json::to_string(&content).unwrap()
    }
}

impl WritePrettyOnStdout for Json {
    fn format_pretty<T: Serialize>(content: T) -> String {
        serde_json::to_string_pretty(&content).unwrap()
    }
}

impl WriteFile for Json {
    const EXTENSION: &'static str = ".json";

    fn with_writer<T: Serialize>(content: T, path: PathBuf, output_path: &Path) {
        serde_json::to_writer(Self::open_file(path, output_path), &content).unwrap()
    }
}

impl WritePrettyFile for Json {
    fn with_pretty_writer<T: Serialize>(
        content: T,
        path: PathBuf,
        output_path: &Path,
        pretty: bool,
    ) {
        if pretty {
            serde_json::to_writer_pretty(Self::open_file(path, output_path), &content).unwrap();
        } else {
            Self::with_writer(content, path, output_path);
        }
    }
}

/// [TOON](https://github.com/toon-format/toon), for a report that goes into
/// an LLM's context rather than a program's parser: the same data as the JSON,
/// spelled to cost fewer tokens. It has one shape, so `--pr` says nothing here.
struct Toon;

impl WriteOnStdout for Toon {
    fn format<T: Serialize>(content: T) -> String {
        toon_format::encode(&content, &toon_format::EncodeOptions::default()).unwrap()
    }
}

impl WriteFile for Toon {
    const EXTENSION: &'static str = ".toon";

    fn with_writer<T: Serialize>(content: T, path: PathBuf, output_path: &Path) {
        // The encoder leaves off the final newline; a line-oriented file wants it.
        writeln!(
            Self::open_file(path, output_path),
            "{}",
            Self::format(content)
        )
        .unwrap();
    }
}

struct Cbor;

impl WriteFile for Cbor {
    const EXTENSION: &'static str = ".cbor";

    fn with_writer<T: Serialize>(content: T, path: PathBuf, output_path: &Path) {
        ciborium::into_writer(&content, Self::open_file(path, output_path)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{read_to_string, remove_dir_all};

    use super::*;

    /// A report in miniature: a nested object, and a list of records whose
    /// keys repeat — which is the shape TOON is built to compress.
    #[derive(Serialize)]
    struct Measure {
        name: &'static str,
        value: f64,
        threshold: f64,
    }

    #[derive(Serialize)]
    struct Pillar {
        name: &'static str,
        measures: Vec<Measure>,
    }

    fn a_pillar() -> Pillar {
        Pillar {
            name: "control flow",
            measures: vec![
                Measure {
                    name: "cognitive complexity",
                    value: 10.0,
                    threshold: 15.0,
                },
                Measure {
                    name: "cyclomatic complexity",
                    value: 5.0,
                    threshold: 10.0,
                },
            ],
        }
    }

    #[test]
    fn every_advertised_format_parses() {
        for name in Format::all() {
            assert!(name.parse::<Format>().is_ok(), "{name} does not parse");
        }
        assert!("bson".parse::<Format>().is_err());
    }

    #[test]
    fn toon_is_one_of_them() {
        assert!(Format::all().contains(&"toon"));
        assert!(matches!("toon".parse::<Format>(), Ok(Format::Toon)));
    }

    #[test]
    fn toon_writes_repeated_keys_once_as_a_table_header() {
        let encoded = Toon::format(a_pillar());
        assert_eq!(
            encoded,
            "name: control flow\n\
             measures[2]{name,value,threshold}:\n  \
               cognitive complexity,10,15\n  \
               cyclomatic complexity,5,10"
        );
    }

    #[test]
    fn dumping_toon_writes_one_file_per_source_file() {
        let output = std::env::temp_dir().join("lspf_analysis_toon_dump");
        let _ = remove_dir_all(&output);
        create_dir_all(&output).unwrap();

        Format::Toon.dump_formats(
            a_pillar(),
            PathBuf::from("/src/deep/hover.rs"),
            Some(&output),
            false,
        );

        let written = read_to_string(output.join("src/deep/hover.rs.toon")).unwrap();
        assert_eq!(written, Toon::format(a_pillar()) + "\n");
        remove_dir_all(&output).unwrap();
    }

    #[test]
    fn dumping_cbor_writes_a_file_that_decodes_to_the_same_values() {
        let output = std::env::temp_dir().join("lspf_analysis_cbor_dump");
        let _ = remove_dir_all(&output);
        create_dir_all(&output).unwrap();

        Format::Cbor.dump_formats(
            a_pillar(),
            PathBuf::from("/src/deep/hover.rs"),
            Some(&output),
            false,
        );

        let file = File::open(output.join("src/deep/hover.rs.cbor")).unwrap();
        let decoded: serde_json::Value = ciborium::from_reader(file).unwrap();
        assert_eq!(decoded, serde_json::to_value(a_pillar()).unwrap());
        remove_dir_all(&output).unwrap();
    }
}
