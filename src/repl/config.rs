use config::{Config, FileFormat, Map};
use directories::ProjectDirs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) static ION_SYNTAX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/ion.sublime-syntax"
));
pub(crate) static PARTIQL_SYNTAX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/partiql.sublime-syntax"
));
pub(crate) static DEFAULT_CONFIG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/partiql-cli.toml"
));

pub(crate) struct ReplConfig {
    pub config: Config,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub history_path: PathBuf,
}

pub(crate) fn repl_config() -> ReplConfig {
    let dirs = ProjectDirs::from("org", "partiql", "partiql-cli").expect("project directories");
    let config_dir = dirs.config_dir();
    let cache_dir = dirs.cache_dir();
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");

    let mut conf = config_dir.to_path_buf();
    conf.push("partiql-cli.toml");

    // If the config file does not exist, create it and write the default config into it.
    //    create_new returns `Ok` if it creates and `Err` if the file already exists
    if let Ok(mut f) = OpenOptions::new().write(true).create_new(true).open(&conf) {
        f.write_all(DEFAULT_CONFIG.as_bytes());
    }

    let config = Config::builder()
        .set_default("repl", Map::from([("theme".to_string(), infer_theme())]))
        .expect("default values")
        .add_source(config::File::from_str(DEFAULT_CONFIG, FileFormat::Toml))
        .add_source(config::File::new(conf.to_str().unwrap(), FileFormat::Toml))
        .build()
        .expect("configuration files");

    // History file used to be written to `~/partiql_cli.history`
    // If the new history file doesn't exist, but legacy does, copy legacy to new location
    // TODO remove all the legacy history path stuff after several versions, say v0.6
    let legacy_history_path = shellexpand::tilde("~/partiql_cli.history").to_string();
    let legacy_history_path = Path::new(&legacy_history_path);
    let mut history_path = cache_dir.to_path_buf();
    history_path.push("partiql-cli.history");
    // If the history file does not exist, create it; if legacy exists, copy it into history
    //    create_new returns `Ok` if it creates and `Err` if the file already exists
    if let Ok(mut f) = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&history_path)
    {
        if let Ok(mut lf) = OpenOptions::new().read(true).open(&legacy_history_path) {
            std::io::copy(&mut lf, &mut f).expect("copy legacy history");
        }
    }

    ReplConfig {
        config,
        config_dir: config_dir.to_path_buf(),
        cache_dir: cache_dir.to_path_buf(),
        history_path,
    }
}

fn infer_theme() -> String {
    const TERM_TIMEOUT_MILLIS: u64 = 20;
    let timeout = std::time::Duration::from_millis(TERM_TIMEOUT_MILLIS);
    let theme = termbg::theme(timeout);
    match theme {
        Ok(termbg::Theme::Light) => "light",
        Ok(termbg::Theme::Dark) => "dark",
        _ => "dark",
    }
    .to_string()
}
