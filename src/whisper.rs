use fon::Audio;
use once_cell::sync::OnceCell;
use std::{env, path::PathBuf, sync::Mutex};
use whisper_rs::{
    convert_integer_to_float_audio, convert_stereo_to_mono_audio, FullParams, SamplingStrategy,
    WhisperContext,
};

const MODEL_PATH_ENV: &str = "MODEL_PATH";

const SAMPLE_RATE: u32 = 48000;
const OUT_SAMPLE_RATE: u32 = 16000;

pub static WHISPER_CTX: OnceCell<Mutex<WhisperContext>> = OnceCell::new();

pub fn whisper_init() -> FullParams<'static, 'static> {
    let model_path = PathBuf::from(env::var(MODEL_PATH_ENV).expect("Couldn't get the MODEL_PATH"))
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    let model_path = model_path.as_str();
    let _ctx = WHISPER_CTX
        .get_or_init(|| Mutex::new(WhisperContext::new(model_path).expect("failed to load model")))
        .lock()
        .unwrap();

    let mut params = FullParams::new(SamplingStrategy::Greedy { n_past: 0 });

    // Edit params as needed.
    // Set the number of threads to use to 1.
    params.set_n_threads(1);
    params.set_language("ja");
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    params
}

pub fn whisper(params: FullParams, audio: &Vec<i16>) {
    let mut ctx = WHISPER_CTX.get().unwrap().lock().unwrap();

    let whisper_audio = Audio::<fon::chan::Ch16, 2>::with_i16_buffer(SAMPLE_RATE, audio.as_slice());
    let mut whisper_audio =
        Audio::<fon::chan::Ch32, 1>::with_audio(OUT_SAMPLE_RATE, &whisper_audio);

    if whisper_audio.sample_rate().get() != OUT_SAMPLE_RATE {
        panic!("sample rate must be 16KHz");
    }

    let whisper_audio = whisper_audio.as_f32_slice();

    println!("whisper_audio: {:?}", whisper_audio);

    ctx.full(params, &whisper_audio[..])
        .expect("failed to run model");

    let num_segments = ctx.full_n_segments();

    for i in 0..num_segments {
        // Get the transcribed text and timestamps for the current segment.
        let segment = ctx.full_get_segment_text(i).expect("failed to get segment");
        let start_timestamp = ctx.full_get_segment_t0(i);
        let end_timestamp = ctx.full_get_segment_t1(i);

        // Format the segment information as a string.
        let line = format!("[{} - {}]: {}\n", start_timestamp, end_timestamp, segment);

        println!("line: {}", &line);
    }
}
