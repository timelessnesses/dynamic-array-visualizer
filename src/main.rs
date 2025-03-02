use std::sync::{Arc, Mutex};

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::rwops::RWops;

mod ffmpeg;

macro_rules! debuggery {
    ($($e:expr),+) => {
        {
            #[cfg(debug_assertions)]
            {
                dbg!($($e),+)
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Array {
    growth: f64,
    old_data_size: usize,
    size: usize,
    capacity: usize,
    hard_limit: Option<usize>,
    old_data_appended: usize, // Track how much old data has been appended back,
    resizes: usize,
    copy_operations: usize
}

impl Array {
    fn new(growth: f64, hard_limit: Option<usize>) -> Array {
        Array {
            growth,
            old_data_size: 0,
            size: 0,
            capacity: 1,
            hard_limit,
            old_data_appended: 0,
            copy_operations: 0,
            resizes: 0,
        }
    }

    /// Will return error if capacity is not enough to hold the new data
    /// Will return [`Ok(usize)`] if the data was added successfully and usize is the address of the new data
    fn grow(&mut self) -> Result<usize, ()> {
        let new_size = self.size + 1;
        if new_size > self.capacity {
            return Err(());
        }
        self.size = new_size;
        Ok(self.size)
    }
    
    fn extend(&mut self) {
        self.resizes += 1;
        self.old_data_size = self.size;
        self.capacity = (self.capacity as f64 * self.growth).ceil() as usize;
        
        if let Some(limit) = self.hard_limit {
            if self.capacity > limit {
                self.capacity = limit;
            }
        }
        self.old_data_appended = 0;
    }

    fn append_old_data(&mut self) -> Result<usize, ()> {
        if self.old_data_appended < self.old_data_size {
            self.copy_operations += 1;
            self.old_data_appended += 1;
            Ok(self.old_data_appended)
        } else {
            Err(())
        }
    }
}

fn main() {

    let cell_size = 10usize;
    let grid_width = (1000 / cell_size) as usize;
    let grid_height = (1000 / cell_size) as usize;

    let mut array = Array::new(std::env::args().nth(1).unwrap_or("1.618".to_string()).parse::<f64>().unwrap(), Some(grid_height * grid_width));
    let ctx = sdl2::init().unwrap();
    let video = ctx.video().unwrap();
    let mut event_pump = ctx.event_pump().unwrap();
    let window = video.window("Array", 1600, 1000).position_centered().build().unwrap();
    let mut canvas = window.into_canvas().accelerated().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let ttf = sdl2::ttf::init().unwrap();
    let font = ttf.load_font_from_rwops(RWops::from_bytes(include_bytes!("../Sen-Regular.ttf")).unwrap(), 30).unwrap();

    let mut operations_per_append = 0.0;
    let mut memory_efficiency = 0.0;
    let mut operations = 0;

    let ffmpeg = Arc::new(Mutex::new(ffmpeg::VideoRecorder::new(&(std::env::args().nth(1).unwrap_or("2.0".to_string()) + ".mp4"), 1600, 1000, 60)));
    let cloned_vr = std::sync::Arc::clone(&ffmpeg.clone());
    println!("Recording will start once started simulation...");
    ctrlc::set_handler(move || {
        cloned_vr.lock().unwrap().kill();
    })
    .expect("Failed to listen for CTRL-C (Force exiting with FFMpeg)");

    // fps stuff
    let mut ft = std::time::Instant::now(); // frame time
    let mut fc = 0; // frame count
    let mut fps = 0.0; // frame per sec
    let mut mf = 0.0; // maximum fps
    let mut lf = 0.0; // minimum fps (shows on screen)
    let mut lpf = 0.0; // act as a cache
    let mut lft = std::time::Instant::now(); // minimum frame refresh time thingy

    let mut all_efficiencies = vec![];
    let mut all_appends = vec![];

    let mut limited_reached = false;
    let mut last_limit_reached = std::time::Instant::now();
    
    'running: loop {
        for event in event_pump.poll_iter() {
            if let sdl2::event::Event::Quit {..} = event { break 'running }
        }

        if last_limit_reached.elapsed().as_secs() >= 3 && limited_reached {
            break 'running;
        }

        canvas.clear();
        
        
        memory_efficiency = ((array.size as f64 - array.old_data_size as f64) + array.old_data_appended as f64) / (array.capacity as f64);
        if !limited_reached {
            all_efficiencies.push(memory_efficiency);
        }
        
        for x in 0..grid_width {
            for y in 0..grid_height {
                let rect = Rect::new(x as i32 * cell_size as i32, y as i32 * cell_size as i32, cell_size as u32, cell_size as u32);
                let index = x + y * grid_width;
                if array.capacity >= index {
                    // are in range of allocated memory
                    // checks if data size is in range of the position
                    if array.size >= index && array.old_data_size <= index { // are not old data
                        canvas.set_draw_color(Color::GREEN);
                    } else if array.size >= index && array.old_data_size >= index { // are old data
                        if index <= array.old_data_appended && !limited_reached {
                            canvas.set_draw_color(Color::CYAN);
                        } else {
                            canvas.set_draw_color(Color::BLUE);
                        }
                    } else { // still empty space
                        canvas.set_draw_color(Color::BLACK);
                    }
                    canvas.fill_rect(rect).unwrap();
                }
            }
        }
        
        match array.grow() {
            Err(_) => {
                if array.old_data_appended == array.old_data_size {
                    println!("\rExpanding array's capacity by allocating more memory");
                    array.extend();
                    println!("New capacity: {}", array.capacity);
                    if let Err(_) = array.grow() {
                        if !limited_reached {
                            limited_reached = true;
                            last_limit_reached = std::time::Instant::now();
                        }
                    }
                    operations += 2;
                }
            },
            Ok(_) => {
                print!("\rSuccessfully appended new data: {}", array.size);
                operations += 1;
            }
        }

        match array.append_old_data() {
            Ok(_) => {
                if !limited_reached {
                    debuggery!("\rSuccessfully appended old data: {}", array.old_data_appended);
                    operations += 1;
                }
            },
            Err(_) => {
            }
        }

        if !limited_reached {
            operations_per_append = operations as f64 / 1.0;
            operations = 0;
            all_appends.push(operations_per_append);
        }

        let mut starting_y = (canvas.logical_size().1 / {
            #[cfg(debug_assertions)]
            {
                10
            }
            #[cfg(not(debug_assertions))]
            {
                7
            }
        }) as i32 + ((font.size_of("a").unwrap().1) * {
            #[cfg(debug_assertions)]
            {
                10
            }
            #[cfg(not(debug_assertions))]
            {
                7
            }
        } / 2) as i32;

        let mem_eff = font.render(&format!("Memory efficiency: {:.3}%", memory_efficiency * 100.0)).blended(Color::BLACK).unwrap();
        let op_append = font.render(&format!("Operations per append: {:.3}", operations_per_append)).blended(Color::BLACK).unwrap();
        let capacity = font.render(&format!("Capacity: {}", array.capacity)).blended(Color::BLACK).unwrap();
        let size = font.render(&format!("Size: {}", array.size)).blended(Color::BLACK).unwrap();
        let gf = font.render(&format!("Growth factor: {}", array.growth)).blended(Color::BLACK).unwrap();
        let all_eff = font.render(&format!("All efficiencies: {:.3}%", all_efficiencies.iter().sum::<f64>() / all_efficiencies.len() as f64 * 100.0)).blended(Color::BLACK).unwrap();
        let all_append = font.render(&format!("All appends: {:.3}", all_appends.iter().sum::<f64>() / all_appends.len() as f64)).blended(Color::BLACK).unwrap();
        let copy_operations = font.render(&format!("Copy operations: {}", array.copy_operations)).blended(Color::BLACK).unwrap();
        let resizes = font.render(&format!("Resizes: {}", array.resizes)).blended(Color::BLACK).unwrap();
        let copy_ops_per_resize = font.render(&format!("Copy operations per resize: {:.3}", array.copy_operations as f64 / array.resizes as f64)).blended(Color::BLACK).unwrap();

        canvas.copy(&mem_eff.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, mem_eff.width(), mem_eff.height()))).unwrap();
        starting_y += mem_eff.height() as i32;
        canvas.copy(&op_append.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, op_append.width(), op_append.height()))).unwrap();
        starting_y += op_append.height() as i32;
        canvas.copy(&capacity.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, capacity.width(), capacity.height()))).unwrap();
        starting_y += capacity.height() as i32;
        canvas.copy(&size.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, size.width(), size.height()))).unwrap();
        starting_y += size.height() as i32;
        canvas.copy(&gf.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, gf.width(), gf.height()))).unwrap();
        starting_y += gf.height() as i32;
        canvas.copy(&all_eff.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, all_eff.width(), all_eff.height()))).unwrap();
        starting_y += all_eff.height() as i32;
        canvas.copy(&all_append.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, all_append.width(), all_append.height()))).unwrap();
        starting_y += all_append.height() as i32;
        canvas.copy(&copy_operations.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, copy_operations.width(), copy_operations.height()))).unwrap();
        starting_y += copy_operations.height() as i32;
        canvas.copy(&resizes.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, resizes.width(), resizes.height()))).unwrap();
        starting_y += resizes.height() as i32;
        canvas.copy(&copy_ops_per_resize.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, copy_ops_per_resize.width(), copy_ops_per_resize.height()))).unwrap();
        starting_y += copy_ops_per_resize.height() as i32;

        #[cfg(debug_assertions)]

        {
            let min_fps = font.render(&format!("Minimum FPS: {:.2}", lf)).blended(Color::BLACK).unwrap();
            let max_fps = font.render(&format!("Maximum FPS: {:.2}", mf)).blended(Color::BLACK).unwrap();
            let cur_fps = font.render(&format!("Current FPS: {:.2}", fps)).blended(Color::BLACK).unwrap();
            canvas.copy(&min_fps.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, min_fps.width(), min_fps.height()))).unwrap();
            starting_y += min_fps.height() as i32;
            canvas.copy(&max_fps.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, max_fps.width(), max_fps.height()))).unwrap();
            starting_y += max_fps.height() as i32;
            canvas.copy(&cur_fps.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1000, starting_y, cur_fps.width(), cur_fps.height()))).unwrap();
            starting_y += cur_fps.height() as i32;
        };

        canvas.set_draw_color(Color::WHITE);

        for x in 0..=grid_width {
            let x_pos = x * cell_size;
            canvas.draw_line((x_pos as i32, 0), (x_pos as i32, 1000)).unwrap();
        }
        for y in 0..=grid_height {
            let y_pos = y * cell_size;
            canvas.draw_line((0, y_pos as i32), (1000, y_pos as i32)).unwrap();
        }

        canvas.set_draw_color(Color::GRAY);

        canvas.present();

        fc += 1;
        let elapsed_time = ft.elapsed();
        if elapsed_time.as_secs() >= 1 {
            fps = fc as f64 / elapsed_time.as_secs_f64();
            fc = 0;
            ft = std::time::Instant::now();
            if fps > mf {
                mf = fps
            } else if fps < lpf {
                lpf = fps
            }
        }
        let elapsed_time = lft.elapsed();
        if elapsed_time.as_secs() >= 3 {
            lf = lpf;
            lpf = fps;
            lft = std::time::Instant::now();
        }
        let mut v = ffmpeg.lock().unwrap();
                v.process_frame(
                    canvas
                        .read_pixels(
                            sdl2::rect::Rect::new(0, 0, 1600, 1000),
                            sdl2::pixels::PixelFormatEnum::RGB24,
                        )
                        .unwrap(),
                );
    }
        let mut a = ffmpeg.lock().unwrap();
        a.done();
}