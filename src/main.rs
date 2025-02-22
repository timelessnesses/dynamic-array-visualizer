use sdl2;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::rwops::RWops;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Array {
    growth: f64,
    old_data_size: usize,
    size: usize,
    capacity: usize,
    hard_limit: Option<usize>,
    old_data_appended: usize, // Track how much old data has been appended back
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
        }
    }

    /// Will return error if capacity is not enough to hold the new data
    /// Will return [`Ok(usize)`] if the data was added successfully and usize is the address of the new data
    fn grow(&mut self) -> Result<usize, ()> {
        let new_size = self.size + 1;
        if new_size > self.capacity {
            return Err(());
        }
        self.size = new_size as usize;
        Ok(self.size)
    }
    
    fn extend(&mut self) {
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
            self.old_data_appended += 1;
            Ok(self.old_data_appended)
        } else {
            Err(())
        }
    }
}

fn main() {
    let mut array = Array::new(std::env::args().nth(1).unwrap_or("2.0".to_string()).parse::<f64>().unwrap(), Some(512 * 512));
    let ctx = sdl2::init().unwrap();
    let video = ctx.video().unwrap();
    let mut event_pump = ctx.event_pump().unwrap();
    let window = video.window("Array", 1600, 1024).position_centered().build().unwrap();
    let mut canvas = window.into_canvas().accelerated().index(5).build().unwrap();
    let texture_creator = canvas.texture_creator();

    let ttf = sdl2::ttf::init().unwrap();
    let font = ttf.load_font_from_rwops(RWops::from_bytes(include_bytes!("../NotoSans-Thin.ttf")).unwrap(), 30).unwrap();

    let mut operations_per_append = 0.0;
    let mut memory_efficiency = 0.0;
    let mut operations = 0;

    // fps stuff
    let mut ft = std::time::Instant::now(); // frame time
    let mut fc = 0; // frame count
    let mut fps = 0.0; // frame per sec
    let mut mf = 0.0; // maximum fps
    let mut lf = 0.0; // minimum fps (shows on screen)
    let mut lpf = 0.0; // act as a cache
    let mut lft = std::time::Instant::now(); // minimum frame refresh time thingy
    
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit {..} => break 'running,
                _ => {}
            }
        }
        canvas.clear();
        
        // Draw the grid
        canvas.set_draw_color(Color::WHITE); // Gray color for grid lines
        for x in 0..512 {
            let x_pos = x as i32 * 2;
            canvas.draw_line((x_pos, 0), (x_pos, 1024)).unwrap();
        }
        for y in 0..512 {
            let y_pos = y as i32 * 2;
            canvas.draw_line((0, y_pos), (1024, y_pos)).unwrap();
        }
        memory_efficiency = array.size as f64 / array.capacity as f64;
        
        // Draw the array state
        for x in 0..512 {
            for y in 0..512 {
                let rect = Rect::new(x as i32 * 2, y as i32 * 2, 2, 2);
                let index = x + y * 512;
                if array.capacity > index {
                    // are in range of allocated memory
                    // checks if data size is in range of the position
                    if array.size >= index && array.old_data_size <= index { // are not old data
                        canvas.set_draw_color(Color::GREEN);
                    } else if array.size >= index && array.old_data_size >= index { // are old data
                        canvas.set_draw_color(Color::BLUE);
                    } else { // still empty space
                        canvas.set_draw_color(Color::BLACK);
                    }
                    canvas.fill_rect(rect).unwrap();
                }
            }
        }
        
        canvas.set_draw_color(Color::GRAY);
        match array.grow() {
            Err(_) => {
                println!("\rExpanding array's capacity by allocating more memory");
                array.extend();
                println!("New capacity: {}", array.capacity);
                array.grow().unwrap();
                operations += 2;
            },
            Ok(_) => {
                print!("\rSuccessfully appended new data: {}", array.size);
                operations += 1;
            }
        }

        // Append old data back into the array
        match array.append_old_data() {
            Ok(_) => {
                print!("\rSuccessfully appended old data: {}", array.old_data_appended);
                operations += 1;
            },
            Err(_) => {
                // No more old data to append
            }
        }

        operations_per_append = operations as f64 / 1.0;
        operations = 0;

        let mem_eff = font.render(&format!("Memory efficiency: {:.2}%", memory_efficiency * 100.0)).blended(Color::BLACK).unwrap();
        let op_append = font.render(&format!("Operations per append: {:.2}", operations_per_append)).blended(Color::BLACK).unwrap();
        let min_fps = font.render(&format!("Minimum FPS: {:.2}", lf)).blended(Color::BLACK).unwrap();
        let max_fps = font.render(&format!("Maximum FPS: {:.2}", mf)).blended(Color::BLACK).unwrap();
        let cur_fps = font.render(&format!("Current FPS: {:.2}", fps)).blended(Color::BLACK).unwrap();
        let capacity = font.render(&format!("Capacity: {}", array.capacity)).blended(Color::BLACK).unwrap();
        let size = font.render(&format!("Size: {}", array.size)).blended(Color::BLACK).unwrap();
        let gf = font.render(&format!("Growth factor: {}", array.growth)).blended(Color::BLACK).unwrap();
        canvas.copy(&mem_eff.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 200, mem_eff.width(), mem_eff.height()))).unwrap();
        canvas.copy(&op_append.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 250, op_append.width(), op_append.height()))).unwrap();
        canvas.copy(&min_fps.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 300, min_fps.width(), min_fps.height()))).unwrap();
        canvas.copy(&max_fps.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 350, max_fps.width(), max_fps.height()))).unwrap();
        canvas.copy(&cur_fps.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 400, cur_fps.width(), cur_fps.height()))).unwrap();
        canvas.copy(&capacity.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 450, capacity.width(), capacity.height()))).unwrap();
        canvas.copy(&size.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 500, size.width(), size.height()))).unwrap();
        canvas.copy(&gf.as_texture(&texture_creator).unwrap(), None, Some(Rect::new(1100, 550, gf.width(), gf.height()))).unwrap();
        
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
        // std::thread::sleep(std::time::Duration::from_millis(10));
    }
}