//! Kit component icons, the few extra Lucide icons this application uses and the
//! application logo. `icon_assets!` embeds only the listed SVGs, not the whole catalog.
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
        FolderCog,
        FolderPlus,
        GripVertical,
        Image,
        Languages,
        Layers,
        Link,
        ListChecks,
        ListOrdered,
        Lock,
        MousePointerClick,
        Package,
        Puzzle,
        Save,
        SearchX,
        Share2,
        ShieldAlert,
        Swords,
        Tag,
    ]
);

/// Asset path of the logo; the same SVG is the source of the executable's icon.
pub const LOGO: &str = "brand/logo.svg";

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == LOGO {
            return Ok(Some(Cow::Borrowed(include_bytes!("../assets/logo.svg"))));
        }
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        if LOGO.starts_with(path) {
            paths.push(LOGO.into());
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
