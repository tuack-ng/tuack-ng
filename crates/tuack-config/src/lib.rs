pub mod config;
pub mod current_location;
pub mod prelude;

pub use config::problem::*;
pub use config::{
    CONFIG_FILE_NAME, CONFIG_MIN_VERSION, CONFIG_VERSION, Config, FileView, FullView, load_config,
    save_config,
};
pub use config::{ContestConfig, ContestDayConfig};
pub use config::{contest, contestday, lang, migrate, msgs, problem};
pub use current_location::CurrentLocation;
