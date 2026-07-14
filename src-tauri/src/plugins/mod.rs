//! Universal plugin system for clipboard content processing.
//!
//! ## Protocol
//! Input (JSON, stdin for external scripts):
//! ```json
//! { "content": "hello", "type": "text" }
//! ```
//! `type` is `"text"` | `"img"` (extensible).
//!
//! Output (JSON, stdout):
//! ```json
//! { "ok": true, "title": "英→汉", "body": "你好", "preview": "你好", "hint": "ECDICT" }
//! ```
//! or `{ "ok": false, "error": "..." }`
//!
//! External runtimes: `node`, `python`, `go`, `shell` — see `plugin.json` manifest.

mod ecdict;
mod manifest;
mod ocr_plugin;
mod registry;
mod runner;
mod samples;
mod translate;

pub use ecdict::{ecdict_ready, install_ecdict, EcdictStatus};
pub use manifest::PluginInfo;
pub use registry::{import_plugin_dir, list_plugins, remove_plugin, set_plugin_enabled};
pub use runner::{run_all_enabled, run_plugin, PluginInput, PluginOutput};
pub use samples::{export_all_samples, export_sample_pack, list_sample_packs, SamplePack};
