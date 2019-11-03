use crate::error::Error;
use crate::logic::Command;
use crate::world::{PlantEditor, Screen, World};
use anyhow::Result;
use crossbeam_channel::Sender;
use sdl2::{
    pixels::Color,
    rect::{Point, Rect},
    render::{Canvas, Texture, TextureQuery},
    video::Window,
    EventPump,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 800;
const TITLE: &str = "Sound Garden";
const TARGET_FPS: u32 = 60;
const TARGET_FRAME_DURATION_NS: u32 = 1_000_000_000u32 / TARGET_FPS;
const REGULAR_FONT: &str = "dat/fnt/Agave-Regular.ttf";
const CHAR_SIZE: u16 = 24;

pub fn main(world: Arc<Mutex<World>>, tx: Sender<Command>) -> Result<()> {
    let sdl_ctx = sdl2::init().map_err(|s| Error::SDLInit(s))?;
    let window = sdl_ctx
        .video()
        .map_err(|s| Error::Video(s))?
        .window(TITLE, WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .opengl()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let mut event_pump = sdl_ctx.event_pump().map_err(|s| Error::EventPump(s))?;
    let ttf_ctx = sdl2::ttf::init()?;

    let main_fnt = ttf_ctx
        .load_font(REGULAR_FONT, CHAR_SIZE)
        .map_err(|s| Error::LoadFont(s))?;

    let texture_creator = canvas.texture_creator();
    let mut char_cache = HashMap::new();

    for c in (0x20..0x7e).map(|c| char::from(c)) {
        let surface = main_fnt.render_char(c).blended(Color::RGB(0, 0, 0))?;
        let texture = texture_creator.create_texture_from_surface(surface)?;
        char_cache.insert(c, texture);
    }

    world.lock().unwrap().cell_size = main_fnt.size_of_char('M')?;

    // Start with a blank canvas.
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();
    canvas.present();

    let target_frame_duration = Duration::new(0, TARGET_FRAME_DURATION_NS);
    let frame_budget = |frame_start: Instant| {
        let frame_duration = frame_start.elapsed();
        if frame_duration < target_frame_duration {
            Some(target_frame_duration - frame_duration)
        } else {
            None
        }
    };

    loop {
        let frame_start = Instant::now();

        process_events(&mut event_pump, &tx)?;

        render_world(&mut canvas, &char_cache, &world.lock().unwrap())?;

        if let Some(budget) = frame_budget(frame_start) {
            std::thread::sleep(budget);
        }
    }
}

fn render_world(
    canvas: &mut Canvas<Window>,
    char_cache: &HashMap<char, Texture>,
    world: &World,
) -> Result<()> {
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();

    // Update & draw stuff.
    let cell_size = world.cell_size;
    match &world.screen {
        Screen::Garden => {
            for p in &world.plants {
                render_char(
                    canvas,
                    &char_cache,
                    p.symbol,
                    Point::new(p.position.x, p.position.y),
                    cell_size,
                )?;
            }
            let p = &world.garden.anima_position;
            render_char(canvas, &char_cache, '@', Point::new(p.x, p.y), cell_size)?;
        }
        Screen::Plant(PlantEditor {
            ix,
            cursor_position,
            ..
        }) => {
            let p = &world.plants[*ix];
            for node in &p.nodes {
                let p = &node.position;
                render_str(
                    canvas,
                    &char_cache,
                    &node.op,
                    Point::new(p.x, p.y),
                    cell_size,
                )?;
            }
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            for (i, j) in &p.edges {
                let n1 = &p.nodes[*i];
                let n2 = &p.nodes[*j];
                canvas
                    .draw_line(
                        Point::from((
                            n1.position.x * (cell_size.0 as i32) + (cell_size.0 as i32) / 2,
                            (n1.position.y + 1) * (cell_size.1 as i32),
                        )),
                        Point::from((
                            n2.position.x * (cell_size.0 as i32) + (cell_size.0 as i32) / 2,
                            n2.position.y * (cell_size.1 as i32),
                        )),
                    )
                    .map_err(|s| Error::Draw(s))?;
            }
            let p = cursor_position;
            render_char(canvas, &char_cache, '_', Point::new(p.x, p.y), cell_size)?;
        }
    }

    // Flip!
    canvas.present();
    Ok(())
}

fn process_events(event_pump: &mut EventPump, tx: &Sender<Command>) -> Result<()> {
    for event in event_pump.poll_iter() {
        tx.send(Command::SDLEvent(event))?;
    }
    Ok(())
}

fn render_char(
    canvas: &mut Canvas<Window>,
    char_cache: &HashMap<char, Texture>,
    ch: char,
    topleft: Point,
    cell_size: (u32, u32),
) -> Result<()> {
    let texture = char_cache.get(&ch).unwrap();
    let TextureQuery { width, height, .. } = texture.query();
    canvas
        .copy(
            &texture,
            None,
            Some(Rect::new(
                topleft.x * (cell_size.0 as i32),
                topleft.y * (cell_size.1 as i32),
                width,
                height,
            )),
        )
        .map_err(|s| Error::TextureCopy(s))?;
    Ok(())
}

fn render_str(
    canvas: &mut Canvas<Window>,
    char_cache: &HashMap<char, Texture>,
    s: &str,
    topleft: Point,
    cell_size: (u32, u32),
) -> Result<()> {
    let mut topleft = topleft.clone();
    for c in s.chars() {
        render_char(canvas, char_cache, c, topleft, cell_size)?;
        topleft.x += 1;
    }
    Ok(())
}
