//! The first start's welcome: a notice in the corner (`crate::notice`, 30 s) says the files
//! stay where they are and how to make and hide fences, and its button opens the user's
//! Styles layer (also on Settings › 「关于」).

use super::*;

impl App {
    pub(super) fn offer_welcome(&mut self) {
        match crate::notice::Notice::show(
            &self.ctx,
            coredesk_core::i18n::text("桌面整理好了"),
            coredesk_core::i18n::text(
                "文件都没有移动，只是显示在栅栏里。\n在桌面空白处拖出一个框可新建栅栏，双击空白处可隐藏或显示栅栏。",
            ),
            coredesk_core::i18n::text("打开样式文件夹"),
            crate::notice::locale(),
        ) {
            Ok(notice) => self.welcome = Some(notice),
            Err(error) => tracing::warn!(%error, "welcome notice not shown"),
        }
    }

    /// The user's Styles layer in Explorer — the welcome notice and the
    /// About card both land modders here; created on demand so the folder
    /// exists even before the first file is dropped in.
    pub(super) fn open_styles_folder(&self) {
        let dir = coredesk_core::styles_fs::user_styles_root().unwrap_or_else(|| {
            self.state
                .config_path_on_disk()
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("Styles")
        });
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
    }
}
