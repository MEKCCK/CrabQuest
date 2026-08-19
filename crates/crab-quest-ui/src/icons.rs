//! 本地图标库：精选 Tabler Icons 的本地 SVG 源与预栅格化 PNG。
//!
//! 运行时只通过 `include_bytes!` 使用 PNG，不访问网络；SVG 源保留在
//! `assets/icons/tabler-svg/`，便于后续按需要重新导出更高分辨率资源。
//! 许可证全文见 `assets/icons/TABLER-ICONS-MIT.txt`。

use std::collections::HashMap;

use eframe::egui;

/// 游戏核心机制对应的统一线性图标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Map,
    Code,
    Hint,
    Heart,
    Xp,
    Combo,
    Boss,
    Achievement,
    Settings,
    Play,
    Passed,
    Locked,
}

impl Icon {
    pub const ALL: [Self; 12] = [
        Self::Map,
        Self::Code,
        Self::Hint,
        Self::Heart,
        Self::Xp,
        Self::Combo,
        Self::Boss,
        Self::Achievement,
        Self::Settings,
        Self::Play,
        Self::Passed,
        Self::Locked,
    ];

    /// 稳定的资源名；可用于测试、日志和 UI 可访问性说明。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Map => "map-2",
            Self::Code => "code",
            Self::Hint => "bulb",
            Self::Heart => "heart",
            Self::Xp => "bolt",
            Self::Combo => "flame",
            Self::Boss => "sword",
            Self::Achievement => "trophy",
            Self::Settings => "settings",
            Self::Play => "player-play",
            Self::Passed => "circle-check",
            Self::Locked => "lock",
        }
    }

    const fn png(self) -> &'static [u8] {
        match self {
            Self::Map => include_bytes!("../assets/icons/tabler-png/tabler-map-2.png"),
            Self::Code => include_bytes!("../assets/icons/tabler-png/tabler-code.png"),
            Self::Hint => include_bytes!("../assets/icons/tabler-png/tabler-bulb.png"),
            Self::Heart => include_bytes!("../assets/icons/tabler-png/tabler-heart.png"),
            Self::Xp => include_bytes!("../assets/icons/tabler-png/tabler-bolt.png"),
            Self::Combo => include_bytes!("../assets/icons/tabler-png/tabler-flame.png"),
            Self::Boss => include_bytes!("../assets/icons/tabler-png/tabler-sword.png"),
            Self::Achievement => include_bytes!("../assets/icons/tabler-png/tabler-trophy.png"),
            Self::Settings => include_bytes!("../assets/icons/tabler-png/tabler-settings.png"),
            Self::Play => include_bytes!("../assets/icons/tabler-png/tabler-player-play.png"),
            Self::Passed => include_bytes!("../assets/icons/tabler-png/tabler-circle-check.png"),
            Self::Locked => include_bytes!("../assets/icons/tabler-png/tabler-lock.png"),
        }
    }
}

/// 懒加载的 egui 纹理缓存。只会从编译进可执行文件的 PNG 解码，不会读文件或联网。
#[derive(Default)]
pub struct IconLibrary {
    textures: HashMap<Icon, egui::TextureHandle>,
}

impl IconLibrary {
    fn texture(&mut self, ctx: &egui::Context, icon: Icon) -> egui::TextureHandle {
        if let Some(texture) = self.textures.get(&icon) {
            return texture.clone();
        }

        let decoded = image::load_from_memory(icon.png())
            .expect("内嵌 Tabler PNG 必须可解码")
            .into_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let pixels = decoded.into_raw();
        let texture = ctx.load_texture(
            format!("tabler-icon-{}", icon.name()),
            egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(icon, texture.clone());
        texture
    }

    /// 在任意 egui 布局中绘制指定图标。
    ///
    /// 图标 PNG 本身是白色描边；通过 `tint` 应用主题色，保证状态不依赖外部资源。
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        icon: Icon,
        size: f32,
        tint: egui::Color32,
    ) -> egui::Response {
        let texture = self.texture(ui.ctx(), icon);
        ui.add(
            egui::Image::new(&texture)
                .fit_to_exact_size(egui::vec2(size, size))
                .tint(tint),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn every_embedded_icon_is_a_48px_png() {
        for icon in Icon::ALL {
            let image = image::load_from_memory(icon.png())
                .unwrap_or_else(|e| panic!("{} 图标无法解码: {e}", icon.name()));
            assert_eq!(image.width(), 48, "{} 宽度", icon.name());
            assert_eq!(image.height(), 48, "{} 高度", icon.name());
        }
    }

    #[test]
    fn icon_names_are_unique() {
        let mut names: Vec<_> = Icon::ALL.iter().map(|icon| icon.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Icon::ALL.len());
    }
}
