//! Kit component icons plus the few extra Lucide icons this application uses.
//! `icon_assets!` embeds only the listed SVGs instead of the whole catalog.
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        ArrowDownToLine,
        ArrowUpToLine,
        Clock,
        Download,
        Film,
        FolderPlus,
        GripVertical,
        Image,
        Link,
        ListChecks,
        Lock,
        Package,
        Puzzle,
        Save,
        ShieldAlert,
        Swords,
        Tag,
    ]
);

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
