/// FFMpeg rendering module
use std::{
    self,
    io::{BufRead, Write},
};
/// [`VideoRecorder`] struct for wrapping around FFMpeg for rendering video by passing frames in [`Vec<u8>`]
pub struct VideoRecorder {
    ffmpeg: std::process::Child,
    status_receiver: std::sync::mpsc::Receiver<String>,
    frame_count: u128,
}

/// Storing FFMpeg informations on current rendering.  
/// frame= 4852 fps=7.0 q=-1.0 Lsize=   37966kB time=00:01:20.81 bitrate=3848.4kbits/s speed=0.117x  
/// Going for biggest data type I can do
#[derive(Debug)]
pub struct FFMpegStatus {
    pub done: bool,
    pub frame: u128,
    pub fps: f64,
    pub quantizer: f64,
    pub time: std::time::Duration,
    pub speed: f64,
    pub progress: f64,
}

impl Default for FFMpegStatus {
    fn default() -> Self {
        Self {
            done: false,
            frame: 0,
            fps: 0.0,
            quantizer: 0.0,
            time: std::time::Duration::new(0, 0),
            speed: 0.0,
            progress: 0.0,
        }
    }
}

/// https://gist.github.com/edwardstock/90b41d4d53af4c32853073865a319222 thanks edward!  
/// ## Usable named groups
/// - `nframe`
/// - `nfps`
/// - `nq`
/// - `nsize`
/// - `ssize`
/// - `sduration`
/// - `nbitrate`
/// - `sbitrate`
/// - `ndup`
/// - `ndrop`
/// - `nspeed`
// const REGEX_IS_FUCKING_HIDEOUS: &str = "frame=\\s*(?<nframe>[0-9]+)\\s+fps=\\s*(?<nfps>[0-9\\.]+)\\s+q=(?<nq>[0-9\\.-]+)\\s+(L?)\\s*size=\\s*(?<nsize>[0-9]+)(?<ssize>kB|mB|b)?\\s*time=\\s*(?<sduration>[0-9\\:\\.]+)\\s*bitrate=\\s*(?<nbitrate>[0-9\\.]+)(?<sbitrate>bits\\/s|mbits\\/s|kbits\\/s)?.*(dup=(?<ndup>\\d+)\\s*)?(drop=(?<ndrop>\\d+)\\s*)?speed=\\s*(?<nspeed>[0-9\\.]+)x";

fn parsery(time: &str) -> std::time::Duration {
    if time == "N/A" {
        return std::time::Duration::new(0, 0);
    }
    let splitter = time.split(":").collect::<Vec<&str>>();
    let hour = splitter[0].trim_start_matches("-").parse::<u64>().unwrap();
    let minute = splitter[1].parse::<u64>().unwrap();
    let seconds = splitter[2].split(".").collect::<Vec<&str>>();
    std::time::Duration::new(
        (hour * 60 * 60) + (minute * 60) + seconds[0].parse::<u64>().unwrap(),
        seconds[1].parse::<u32>().unwrap(),
    )
}

impl VideoRecorder {
    /// Spawns new instance of FFMpeg with out file, size and FPS
    pub fn new(out: &str, width: u32, height: u32, fps: u32) -> Self {
        let mut ffmpeg_cmd = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-s",
                &format!("{}x{}", width, height),
                "-r",
                &format!("{}", fps),
                "-i",
                "pipe:0",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "veryslow",
                "-y",
                "-progress",
                "pipe:1",
                out,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("FFMpeg failed to start");
        let (tx, rx) = std::sync::mpsc::channel();
        let output_lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let ol_cloned = std::sync::Arc::clone(&output_lines);
        let stdout = ffmpeg_cmd.stdout.take().expect("Failed to take STDOUT");
        let r = std::io::BufReader::new(stdout);
        std::thread::spawn(move || {
            for line in r.lines() {
                let l = line.expect("Failed to read line");
                let mut clonery = ol_cloned.lock().unwrap();
                clonery.push(l.clone());
                tx.send(l)
                    .expect("Failed to send FFMpeg line to main thread")
            }
        });
        Self {
            ffmpeg: ffmpeg_cmd,
            status_receiver: rx,
            frame_count: 0,
        }
    }

    /// Function for passing the frames to FFMpeg. This doesn't cost a lot performance.
    pub fn process_frame(&mut self, frame: Vec<u8>) {
        self.ffmpeg
            .stdin
            .as_mut()
            .unwrap()
            .write_all(frame.as_slice())
            .expect("Pipe is closed.");
        self.frame_count += 1;
    }

    pub fn get_render_status(&mut self) -> Option<FFMpegStatus> {
        if let Ok(Some(_)) = self.ffmpeg.try_wait() {
            return Some(FFMpegStatus {
                done: true,
                ..Default::default()
            });
        }
        let mut status = self.status_receiver.try_iter().collect::<Vec<String>>();
        let mut should_return = true;
        for s in &status {
            if s.contains("frame") {
                should_return = false;
                break;
            } else {
                should_return = true;
            }
        }
        if should_return || status.is_empty() {
            return None;
        }
        let status = status
            .iter_mut()
            .map(|x| x.split("=").collect::<Vec<&str>>())
            .collect::<Vec<Vec<&str>>>();
        let mut st = FFMpegStatus {
            ..Default::default()
        };
        for kvs in status {
            let k = kvs[0];
            let v = kvs[1].trim();
            match k.trim() {
                "frame" => st.frame = v.parse().unwrap_or(0),
                "fps" => st.fps = v.parse().unwrap_or(0.0),
                "q" => st.quantizer = v.parse().unwrap_or(0.0),
                "out_time" => st.time = parsery(v),
                "speed" => st.speed = v.trim_end_matches("x").parse().unwrap_or(0.0),
                _ => {}
            }
        }
        st.progress = st.frame as f64 / self.frame_count as f64;
        Some(st)
    }

    /// Finalizing rendering. Wait for FFMpeg to exit
    pub fn done(&mut self) {
        if let Some(stdin) = self.ffmpeg.stdin.take() {
            drop(stdin)
        }
        if let Ok(Some(_)) = self.ffmpeg.try_wait() {
            println!("FFMpeg already exited");
        } else {
            while let Ok(None) = self.ffmpeg.try_wait() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                println!(
                    "Waiting for FFMpeg to exit... (Progress: {}%)",
                    self.get_render_status()
                        .unwrap_or_default()
                        .progress
                        * 100.0
                );
            }
        }
    }

    pub fn kill(&mut self) {
        self.ffmpeg.kill().unwrap();
    }
}