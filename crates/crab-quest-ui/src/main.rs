use crab_quest_core::app::GameApp;
use crab_quest_core::engine::Engine;
use crab_quest_core::level::LevelSet;
use crab_quest_core::sandbox::BwrapSandbox;
use crab_quest_core::save;
use crab_quest_core::ui::UiBackend;
use crab_quest_core::validate::mapper::ErrorMapper;
use crab_quest_ui::GameUi;
use std::collections::HashSet;

/// Applies a real compositor-level alpha to this process' X11 client window.
///
/// Miniquad defaults to X11 on Linux, including when it is hosted by Xwayland
/// in a Wayland desktop.  Wayland itself deliberately has no equivalent
/// cross-desktop window-opacity protocol, so a missing X11 display is an
/// explicit no-op rather than a fake, focus-dependent visual effect.
#[cfg(target_os = "linux")]
mod x11_window_opacity {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void};

    type Window = c_ulong;
    type Atom = c_ulong;
    type Display = c_void;

    const SUCCESS: c_int = 0;
    const PROP_MODE_REPLACE: c_int = 0;
    const MAX_SEARCH_DEPTH: u8 = 3;

    #[link(name = "X11")]
    unsafe extern "C" {
        fn XOpenDisplay(display_name: *const c_char) -> *mut Display;
        fn XCloseDisplay(display: *mut Display) -> c_int;
        fn XDefaultRootWindow(display: *mut Display) -> Window;
        fn XInternAtom(display: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
        fn XQueryTree(
            display: *mut Display,
            window: Window,
            root_return: *mut Window,
            parent_return: *mut Window,
            children_return: *mut *mut Window,
            nchildren_return: *mut c_uint,
        ) -> c_int;
        fn XGetWindowProperty(
            display: *mut Display,
            window: Window,
            property: Atom,
            long_offset: c_long,
            long_length: c_long,
            delete: c_int,
            req_type: Atom,
            actual_type_return: *mut Atom,
            actual_format_return: *mut c_int,
            nitems_return: *mut c_ulong,
            bytes_after_return: *mut c_ulong,
            prop_return: *mut *mut c_uchar,
        ) -> c_int;
        fn XChangeProperty(
            display: *mut Display,
            window: Window,
            property: Atom,
            property_type: Atom,
            format: c_int,
            mode: c_int,
            data: *const c_uchar,
            nelements: c_int,
        ) -> c_int;
        fn XFlush(display: *mut Display) -> c_int;
        fn XFree(data: *mut c_void) -> c_int;
    }

    fn atom(display: *mut Display, name: &str, only_if_exists: bool) -> Atom {
        let name = CString::new(name).expect("X11 atom names contain no NUL");
        // SAFETY: `display` is a live Xlib display for this call and `name` is NUL terminated.
        unsafe { XInternAtom(display, name.as_ptr(), i32::from(only_if_exists)) }
    }

    unsafe fn property_u32(display: *mut Display, window: Window, property: Atom) -> Option<u32> {
        let mut actual_type = 0;
        let mut actual_format = 0;
        let mut item_count = 0;
        let mut bytes_after = 0;
        let mut data = std::ptr::null_mut();
        // SAFETY: all output pointers are valid for the duration of the Xlib call.
        let status = unsafe {
            XGetWindowProperty(
                display,
                window,
                property,
                0,
                1,
                0,
                0,
                &mut actual_type,
                &mut actual_format,
                &mut item_count,
                &mut bytes_after,
                &mut data,
            )
        };
        if status != SUCCESS || data.is_null() || actual_format != 32 || item_count == 0 {
            if !data.is_null() {
                // SAFETY: Xlib allocated `data` and ownership is transferred to this caller.
                unsafe { XFree(data.cast()) };
            }
            return None;
        }
        // Xlib returns 32-bit properties in a C `long`-sized slot.
        let value = unsafe { *(data as *const c_ulong) as u32 };
        // SAFETY: Xlib allocated `data` and ownership is transferred to this caller.
        unsafe { XFree(data.cast()) };
        Some(value)
    }

    unsafe fn property_text(
        display: *mut Display,
        window: Window,
        property: Atom,
    ) -> Option<String> {
        let mut actual_type = 0;
        let mut actual_format = 0;
        let mut item_count = 0;
        let mut bytes_after = 0;
        let mut data = std::ptr::null_mut();
        // SAFETY: all output pointers are valid for the duration of the Xlib call.
        let status = unsafe {
            XGetWindowProperty(
                display,
                window,
                property,
                0,
                2048,
                0,
                0,
                &mut actual_type,
                &mut actual_format,
                &mut item_count,
                &mut bytes_after,
                &mut data,
            )
        };
        if status != SUCCESS || data.is_null() || actual_format != 8 || item_count == 0 {
            if !data.is_null() {
                // SAFETY: Xlib allocated `data` and ownership is transferred to this caller.
                unsafe { XFree(data.cast()) };
            }
            return None;
        }
        // SAFETY: Xlib returned `item_count` 8-bit items in `data`.
        let value = unsafe { std::slice::from_raw_parts(data, item_count as usize).to_vec() };
        // SAFETY: Xlib allocated `data` and ownership is transferred to this caller.
        unsafe { XFree(data.cast()) };
        Some(
            String::from_utf8_lossy(&value)
                .trim_end_matches('\0')
                .to_owned(),
        )
    }

    unsafe fn find_window_for_pid(
        display: *mut Display,
        window: Window,
        pid_property: Atom,
        pid: u32,
        depth: u8,
    ) -> Option<Window> {
        if unsafe { property_u32(display, window, pid_property) } == Some(pid) {
            return Some(window);
        }
        if depth == 0 {
            return None;
        }

        let mut root = 0;
        let mut parent = 0;
        let mut children = std::ptr::null_mut();
        let mut child_count = 0;
        // SAFETY: all output pointers are valid and `display` is live.
        if unsafe {
            XQueryTree(
                display,
                window,
                &mut root,
                &mut parent,
                &mut children,
                &mut child_count,
            )
        } == 0
        {
            return None;
        }
        let child_ids = if children.is_null() {
            Vec::new()
        } else {
            // SAFETY: XQueryTree returned `child_count` elements in `children`.
            let result =
                unsafe { std::slice::from_raw_parts(children, child_count as usize).to_vec() };
            // SAFETY: Xlib allocated the children array for this query.
            unsafe { XFree(children.cast()) };
            result
        };
        for child in child_ids {
            if let Some(found) = unsafe {
                find_window_for_pid(display, child, pid_property, pid, depth.saturating_sub(1))
            } {
                return Some(found);
            }
        }
        None
    }

    unsafe fn find_window_for_title(
        display: *mut Display,
        window: Window,
        title_property: Atom,
        title: &str,
        depth: u8,
    ) -> Option<Window> {
        let mut matching_window =
            (unsafe { property_text(display, window, title_property) }.as_deref() == Some(title))
                .then_some(window);
        if depth == 0 {
            return matching_window;
        }

        let mut root = 0;
        let mut parent = 0;
        let mut children = std::ptr::null_mut();
        let mut child_count = 0;
        // SAFETY: all output pointers are valid and `display` is live.
        if unsafe {
            XQueryTree(
                display,
                window,
                &mut root,
                &mut parent,
                &mut children,
                &mut child_count,
            )
        } == 0
        {
            return matching_window;
        }
        let child_ids = if children.is_null() {
            Vec::new()
        } else {
            // SAFETY: XQueryTree returned `child_count` elements in `children`.
            let result =
                unsafe { std::slice::from_raw_parts(children, child_count as usize).to_vec() };
            // SAFETY: Xlib allocated the children array for this query.
            unsafe { XFree(children.cast()) };
            result
        };
        // Keep the last matching top-level window: newly created client windows
        // are appended there by the window manager, which also handles multiple
        // concurrently launched game instances without touching unrelated apps.
        for child in child_ids {
            if let Some(found) = unsafe {
                find_window_for_title(
                    display,
                    child,
                    title_property,
                    title,
                    depth.saturating_sub(1),
                )
            } {
                matching_window = Some(found);
            }
        }
        matching_window
    }

    pub fn apply(alpha: u32) -> Result<u64, &'static str> {
        // SAFETY: null asks Xlib to use the process' DISPLAY environment value.
        let display = unsafe { XOpenDisplay(std::ptr::null()) };
        if display.is_null() {
            return Err("没有可用的 X11 显示；原生 Wayland 不支持标准窗口透明度属性");
        }

        let result = (|| unsafe {
            let pid_property = atom(display, "_NET_WM_PID", true);
            let cardinal = atom(display, "CARDINAL", false);
            let root = XDefaultRootWindow(display);
            let window = if pid_property == 0 {
                None
            } else {
                find_window_for_pid(
                    display,
                    root,
                    pid_property,
                    std::process::id(),
                    MAX_SEARCH_DEPTH,
                )
            }
            .or_else(|| {
                let title_property = atom(display, "_NET_WM_NAME", true);
                (title_property != 0).then(|| {
                    find_window_for_title(
                        display,
                        root,
                        title_property,
                        "Rust 学习游戏",
                        MAX_SEARCH_DEPTH,
                    )
                })?
            })
            .ok_or("未找到本进程的 X11 顶层窗口")?;
            let opacity_property = atom(display, "_NET_WM_WINDOW_OPACITY", false);
            let value = alpha as c_ulong;
            XChangeProperty(
                display,
                window,
                opacity_property,
                cardinal,
                32,
                PROP_MODE_REPLACE,
                (&value as *const c_ulong).cast(),
                1,
            );
            XFlush(display);
            if property_u32(display, window, opacity_property) == Some(alpha) {
                Ok(window as u64)
            } else {
                Err("窗口透明度属性写入后未能回读验证")
            }
        })();
        // SAFETY: `display` was successfully opened above and is no longer used.
        unsafe { XCloseDisplay(display) };
        result
    }
}

#[cfg(not(target_os = "linux"))]
mod x11_window_opacity {
    pub fn apply(_: u32) -> Result<u64, &'static str> {
        Err("当前平台没有 X11 窗口透明度支持")
    }
}

const FOCUSED_WINDOW_OPACITY: u32 = 0xE0FF_FFFF;

/// 请求带 Alpha 通道的原生 framebuffer；X11/Wayland 合成器可据此显示窗口透明区域。
/// 其余窗口参数保持 macroquad/miniquad 默认值，避免改变平台行为。
fn window_conf() -> macroquad::window::Conf {
    let mut conf = macroquad::window::Conf {
        window_title: "CrabQuest".to_owned(),
        ..Default::default()
    };
    conf.platform.framebuffer_alpha = true;
    conf
}

fn save_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".local/share/crab-quest/save.toml")
}

#[macroquad::main(window_conf)]
async fn main() {
    // This is applied after macroquad has created its native window.  A successful
    // read-back is logged so focused-window transparency is observable rather
    // than being confused with a desktop's inactive-window effect.
    match x11_window_opacity::apply(FOCUSED_WINDOW_OPACITY) {
        Ok(window) => {
            eprintln!("已启用 X11 聚焦窗口透明：window=0x{window:x}，opacity=88%（属性回读已验证）")
        }
        Err(reason) => eprintln!("未启用原生窗口透明：{reason}"),
    }
    let level_set = match LevelSet::load(&crab_quest_data::levels_dir()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("关卡加载失败: {e}");
            std::process::exit(1);
        }
    };
    // P4-26：自定义关卡目录——`--levels <dir>` 覆盖默认用户目录
    // `~/.local/share/crab-quest/levels/`；目录不存在时无自定义章节（行为与现状一致）。
    let custom_dir = crab_quest_data::custom_levels_dir_from_args(std::env::args().skip(1));
    let builtin_ids: HashSet<String> = level_set.levels.iter().map(|l| l.id.clone()).collect();
    let (custom_levels, custom_errors) = crab_quest_core::load_custom_levels(&custom_dir, &builtin_ids);
    for err in &custom_errors {
        // 启动日志：逐文件中文报错；其余文件照常加载，游戏不崩溃
        eprintln!("{}", err.message());
    }
    let save_data = save::load(&save_path()).unwrap_or_default();
    let mapper = match ErrorMapper::load(&crab_quest_data::errors_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("错误码映射加载失败（使用兜底表）: {e}");
            ErrorMapper::default_fallback()
        }
    };
    // P4-24：bwrap 真隔离沙盒。启动时探测一次完整隔离调用；bwrap 缺失或
    // 内核不允许用户命名空间 → 显式中文错误并退出，绝不静默降级到无隔离模式。
    let sandbox = match BwrapSandbox::try_new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let engine = Engine::with_custom_levels(
        level_set,
        custom_levels,
        save_data,
        mapper,
        Box::new(sandbox),
    );
    let mut app = GameApp::with_custom_load_errors(
        engine,
        custom_errors.iter().map(|e| e.message()).collect(),
    );
    let mut ui = GameUi::new();
    // P3-18：通关庆祝「已自动保存」阶段首次显示存档路径
    ui.set_save_path(save_path().display().to_string());
    if let Err(e) = ui.run(&mut app).await {
        eprintln!("运行错误: {e}");
    }
    if let Err(e) = save::save(app.save_ref(), &save_path()) {
        eprintln!("存档失败: {e}");
    }
}
