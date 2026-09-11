//! Embedded assets: app icons plus gpui-kit's default icon set.

use std::borrow::Cow;

use gpui_kit::{AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*.svg"]
pub struct PitypeAssets;

impl AssetSource for PitypeAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("icons/") && path.ends_with(".svg") {
            if let Some(data) = Self::get(path) {
                return Ok(Some(data.data));
            }
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
