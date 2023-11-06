use macroquad::{miniquad::conf::Icon, prelude::*};
use mlua::prelude::*;
use regex::Regex;
use std::{env, fs, path::Path, process::Command};
use walkdir::WalkDir;

// Embedded assets
const ICON_16: &[u8; 1024] = include_bytes!("../assets/icon_16.rgba");
const ICON_32: &[u8; 4096] = include_bytes!("../assets/icon_32.rgba");
const ICON_64: &[u8; 16384] = include_bytes!("../assets/icon_64.rgba");

const LUACHECK: &[u8] = include_bytes!("../assets/luacheck.exe");
const LUAFORMAT: &[u8] = include_bytes!("../assets/lua-format.exe");

const BUMP: &str = include_str!("../assets/bump.lua");
const CLASSIC: &str = include_str!("../assets/classic.lua");
const FLUX: &str = include_str!("../assets/flux.lua");
const INSPECT: &str = include_str!("../assets/inspect.lua");
const JSON: &str = include_str!("../assets/json.lua");
const LUME: &str = include_str!("../assets/lume.lua");
const TICK: &str = include_str!("../assets/tick.lua");
const TINY: &str = include_str!("../assets/tiny.lua");

// Virtual resolution
const VIRTUAL_WIDTH: f32 = 1280.0;
const VIRTUAL_HEIGHT: f32 = 720.0;

// Window configuration
fn window_conf() -> Conf {
    Conf {
        window_title: "Pesto".to_owned(),
        window_width: 960,
        window_height: 540,
        icon: Option::Some(Icon {
            small: ICON_16.to_owned(),
            medium: ICON_32.to_owned(),
            big: ICON_64.to_owned(),
        }),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut error = false;
    let mut error_message: String = "".to_string();

    let directory;

    // Handle command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        directory = ".";
    } else {
        directory = &args[1];
    }

    // Extract luacheck and lua-format if not present
    let luacheck_path = env::temp_dir().join("luacheck.exe");
    let luaformat_path = env::temp_dir().join("lua-format.exe");

    if !luacheck_path.exists() {
        fs::write(&luacheck_path, LUACHECK).unwrap();
    }

    if !luaformat_path.exists() {
        fs::write(&luaformat_path, LUAFORMAT).unwrap();
    }

    // Load lua
    let lua = Lua::new();

    let globals = lua.globals();

    // Setup require search path
    let package_path = env::current_dir().unwrap().join(directory).join("?.lua");

    let package_table: LuaTable = globals.get("package").unwrap();

    package_table
        .set(
            "path",
            format!(
                "{}{}",
                package_table.get::<_, String>("path").unwrap(),
                package_path.to_str().unwrap().to_string()
            ),
        )
        .unwrap();

    // Load api
    let pesto_table = lua.create_table().unwrap();

    let graphics_table = lua.create_table().unwrap();

    let graphics_circle = lua
        .create_function(|_, (x, y, radius): (f32, f32, f32)| {
            draw_circle(x, y, radius, WHITE);

            Ok(())
        })
        .unwrap();

    graphics_table.set("circle", graphics_circle).unwrap();

    pesto_table.set("graphics", graphics_table).unwrap();

    // Load external libraries
    let bump = lua.load(BUMP).eval::<LuaTable>().unwrap();
    let classic = lua.load(CLASSIC).eval::<LuaTable>().unwrap();
    let flux = lua.load(FLUX).eval::<LuaTable>().unwrap();
    let inspect = lua.load(INSPECT).eval::<LuaTable>().unwrap();
    let json = lua.load(JSON).eval::<LuaTable>().unwrap();
    let lume = lua.load(LUME).eval::<LuaTable>().unwrap();
    let tick = lua.load(TICK).eval::<LuaTable>().unwrap();
    let tiny = lua.load(TINY).eval::<LuaTable>().unwrap();

    pesto_table.set("collision", bump).unwrap();
    pesto_table.set("Object", classic).unwrap();
    pesto_table.set("tween", flux).unwrap();
    pesto_table.set("inspect", inspect).unwrap();
    pesto_table.set("json", json).unwrap();
    pesto_table.set("utils", lume).unwrap();
    pesto_table.set("timer", tick).unwrap();
    pesto_table.set("ecs", tiny).unwrap();

    lua.globals().set("pesto", pesto_table).unwrap();

    // Check if main.lua exists in the given directory
    let main_lua_path = Path::new(directory).join("main.lua");

    if !main_lua_path.exists() {
        error = true;
        error_message = "main.lua not found.".to_string()
    }

    if !error {
        // Lint all lua files
        let output = Command::new(luacheck_path)
            .arg(directory)
            .arg("--globals")
            .arg("pesto")
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        let regex = Regex::new(r"(\d+) (warning|warnings) / (\d+) (error|errors)").unwrap();

        if let Some(captures) = regex.captures(&stdout) {
            let warnings = captures[1].parse::<u32>().unwrap();
            let errors = captures[3].parse::<u32>().unwrap();

            if errors > 0 || warnings > 0 {
                error = true;
                error_message = stdout;
            }
        }

        // Format all lua files
        for entry in WalkDir::new(directory) {
            if let Ok(entry) = entry {
                let path = entry.path();

                if path.is_file() && path.extension().unwrap().to_str() == Some("lua") {
                    Command::new(luaformat_path.clone())
                        .arg(path)
                        .arg("-i")
                        .status()
                        .unwrap();
                }
            }
        }
    }

    if !error {
        // Execute main.lua
        let main_lua = fs::read_to_string(main_lua_path).unwrap();

        if let Err(err) = lua.load(main_lua).set_name("main.lua").exec() {
            error = true;
            error_message = err.to_string()
        }
    }

    // Macroquad letterbox setup
    let render_target = render_target(VIRTUAL_WIDTH as u32, VIRTUAL_HEIGHT as u32);
    render_target.texture.set_filter(FilterMode::Nearest);

    let mut render_target_cam =
        Camera2D::from_display_rect(Rect::new(0., 0., VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
    render_target_cam.render_target = Some(render_target.clone());

    // Main loop
    loop {
        // Letterbox update
        let scale: f32 = f32::min(
            screen_width() / VIRTUAL_WIDTH,
            screen_height() / VIRTUAL_HEIGHT,
        );

        let _virtual_mouse_pos = Vec2 {
            x: (mouse_position().0 - (screen_width() - (VIRTUAL_WIDTH * scale)) * 0.5) / scale,
            y: (mouse_position().1 - (screen_height() - (VIRTUAL_HEIGHT * scale)) * 0.5) / scale,
        };

        set_camera(&render_target_cam);

        if error {
            clear_background(SKYBLUE);

            draw_text("ERROR", 10.0, 50.0, 80.0, WHITE);

            let lines: Vec<&str> = error_message.lines().collect();
            let line_height = 50.0;

            for (i, line) in lines.iter().enumerate() {
                let y = i as f32 * line_height;
                draw_text(line, 10.0, 100.0 + y, 32.0, WHITE);
            }
        } else {
            clear_background(BLACK);

            let pesto_table: LuaTable = lua.globals().get("pesto").unwrap();

            match pesto_table.get::<_, LuaFunction>("update") {
                Ok(update_function) => {
                    if let Err(err) = update_function.call::<_, ()>(get_frame_time()) {
                        error = true;
                        error_message = err.to_string()
                    }
                }
                Err(_err) => {
                    error = true;
                    error_message = "Update function not found.".to_string();
                }
            };
        }

        // Draw letterboxed render texture
        set_default_camera();

        if error {
            clear_background(SKYBLUE);
        } else {
            clear_background(LIME);
        }

        draw_texture_ex(
            &render_target.texture,
            (screen_width() - (VIRTUAL_WIDTH * scale)) * 0.5,
            (screen_height() - (VIRTUAL_HEIGHT * scale)) * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(VIRTUAL_WIDTH * scale, VIRTUAL_HEIGHT * scale)),
                flip_y: true,
                ..Default::default()
            },
        );

        next_frame().await;
    }
}
