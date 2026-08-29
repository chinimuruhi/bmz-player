#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppOptions {
    pub boot_play_sample: bool,
    /// Debug: start directly on a synthetic result screen.
    pub boot_result_sample: bool,
    /// beatoraja 互換: 譜面ファイル PATH を指定して起動時プレイ。
    pub boot_play_path: Option<String>,
    pub autoplay_on_start: bool,
    /// uBMplay 互換の外部ビューワー起動 (`-P`)。
    pub viewer_play: bool,
    /// 起動済みの外部ビューワーを停止する (`-S`)。
    pub viewer_stop: bool,
    /// uBMplay 互換の開始小節 (`-N0`, `-N 0`)。
    pub start_measure: Option<u32>,
    /// 起動譜面で Decide を通らず Play へ入る。
    pub skip_decide: bool,
    /// Play 終了後に Result を表示せずプロセスを終了する。
    pub skip_result: bool,
    /// 起動譜面を import して確定した開始時刻。CLI parse 時点では未設定。
    pub boot_start_time_us: Option<i64>,
    /// 外部ビューワーの一時importとPlayで共有するBMS `#RANDOM` seed。
    pub boot_bms_random_seed: Option<u64>,
    pub smoke_exit_after_frames: Option<u32>,
    pub smoke_exit_after_play_frames: Option<u32>,
    pub smoke_exit_after_result_frames: Option<u32>,
    pub smoke_exit_on_result: bool,
    pub smoke_screenshot_path: Option<String>,
    /// `--boot-replay <SLOT>` / `-r1..4` で指定された 0-based のスロット index。
    pub boot_replay_slot: Option<u8>,
    /// `--boot-replay-file <PATH>`: リプレイファイルを直接指定して再生する。
    /// `bmz ir replay` でダウンロードした IR リプレイの再生に使う。
    pub boot_replay_file: Option<String>,
    /// `--boot-course-replay <COURSE_ID>` で指定されたコース id。
    /// 指定された場合、そのコースの最新 attempt を replay 再生する。
    pub boot_course_replay_id: Option<i64>,
    /// `--boot-course <COURSE_ID>` で指定されたコース id。
    /// 指定された場合、そのコースを fresh で起動する。
    pub boot_course_id: Option<i64>,
    /// `--renderer <backend>` で指定されたレンダラーバックエンド。
    pub renderer: Option<RendererBackend>,
    /// `-p` / `--practice`: boot into practice mode (CLI only).
    pub boot_practice: bool,
    pub practice_start_ms: Option<u32>,
    pub practice_end_ms: Option<u32>,
    /// Lua skin developer mode. `Compat` keeps supported function fields in a
    /// persistent runtime VM instead of compiling them at load time.
    pub lua_skin_runtime_mode: bmz_skin::LuaSkinRuntimeMode,
}

impl AppOptions {
    pub fn parse_args<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut options = Self::default();
        let mut args = args.into_iter().peekable();

        while let Some(arg) = args.next() {
            let arg = arg.as_ref();
            if let Some(value) = arg.strip_prefix("--smoke-exit-after-frames=") {
                options.smoke_exit_after_frames = Some(parse_smoke_exit_after_frames_value(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--smoke-exit-after-play-frames=") {
                options.smoke_exit_after_play_frames =
                    Some(parse_smoke_exit_after_play_frames_value(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--smoke-exit-after-result-frames=") {
                options.smoke_exit_after_result_frames =
                    Some(parse_smoke_exit_after_result_frames_value(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--smoke-screenshot=") {
                options.smoke_screenshot_path = Some(parse_smoke_screenshot_path(value)?);
                options.smoke_exit_after_frames.get_or_insert(3);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--boot-replay-file=") {
                options.boot_replay_file = Some(value.to_string());
                continue;
            }
            if let Some(value) = arg.strip_prefix("--boot-replay=") {
                options.boot_replay_slot = Some(parse_boot_replay_slot(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--boot-course-replay=") {
                options.boot_course_replay_id = Some(parse_boot_course_replay_id(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--boot-course=") {
                options.boot_course_id = Some(parse_boot_course_id(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--renderer=") {
                options.renderer = Some(parse_renderer_backend(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--practice-start-ms=") {
                options.practice_start_ms = Some(parse_practice_ms(value, PRACTICE_START_MS_ARG)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--practice-end-ms=") {
                options.practice_end_ms = Some(parse_practice_ms(value, PRACTICE_END_MS_ARG)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix("--lua-skin-runtime=") {
                options.lua_skin_runtime_mode = parse_lua_skin_runtime_mode(value)?;
                continue;
            }
            if let Some(value) = arg.strip_prefix("--start-measure=") {
                options.start_measure = Some(parse_start_measure(value)?);
                continue;
            }
            if let Some(value) = arg.strip_prefix(START_MEASURE_SHORT_ARG)
                && !value.is_empty()
            {
                options.start_measure = Some(parse_start_measure(value)?);
                continue;
            }

            match arg {
                BOOT_PLAY_SAMPLE_ARG => options.boot_play_sample = true,
                BOOT_RESULT_SAMPLE_ARG => options.boot_result_sample = true,
                AUTOPLAY_ON_START_ARG | AUTOPLAY_SHORT_ARG => options.autoplay_on_start = true,
                VIEWER_PLAY_ARG | VIEWER_PLAY_SHORT_ARG => {
                    options.viewer_play = true;
                    options.autoplay_on_start = true;
                    options.skip_decide = true;
                }
                VIEWER_STOP_ARG | VIEWER_STOP_SHORT_ARG => options.viewer_stop = true,
                SKIP_DECIDE_ARG => options.skip_decide = true,
                SKIP_RESULT_ARG => options.skip_result = true,
                START_MEASURE_ARG | START_MEASURE_SHORT_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{arg} requires a measure number");
                    };
                    options.start_measure = Some(parse_start_measure(value.as_ref())?);
                }
                SMOKE_EXIT_ON_RESULT_ARG => options.smoke_exit_on_result = true,
                SMOKE_SCREENSHOT_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{SMOKE_SCREENSHOT_ARG} requires an output path");
                    };
                    options.smoke_screenshot_path =
                        Some(parse_smoke_screenshot_path(value.as_ref())?);
                    options.smoke_exit_after_frames.get_or_insert(3);
                }
                "--help" | "-h" => {}
                SMOKE_EXIT_AFTER_FRAMES_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{SMOKE_EXIT_AFTER_FRAMES_ARG} requires a frame count");
                    };
                    options.smoke_exit_after_frames =
                        Some(parse_smoke_exit_after_frames_value(value.as_ref())?);
                }
                SMOKE_EXIT_AFTER_PLAY_FRAMES_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{SMOKE_EXIT_AFTER_PLAY_FRAMES_ARG} requires a frame count");
                    };
                    options.smoke_exit_after_play_frames =
                        Some(parse_smoke_exit_after_play_frames_value(value.as_ref())?);
                }
                SMOKE_EXIT_AFTER_RESULT_FRAMES_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{SMOKE_EXIT_AFTER_RESULT_FRAMES_ARG} requires a frame count");
                    };
                    options.smoke_exit_after_result_frames =
                        Some(parse_smoke_exit_after_result_frames_value(value.as_ref())?);
                }
                BOOT_REPLAY_FILE_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{BOOT_REPLAY_FILE_ARG} requires a replay file path");
                    };
                    options.boot_replay_file = Some(value.as_ref().to_string());
                }
                BOOT_REPLAY_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{BOOT_REPLAY_ARG} requires a slot number (1..4)");
                    };
                    options.boot_replay_slot = Some(parse_boot_replay_slot(value.as_ref())?);
                }
                BOOT_COURSE_REPLAY_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{BOOT_COURSE_REPLAY_ARG} requires a course id");
                    };
                    options.boot_course_replay_id =
                        Some(parse_boot_course_replay_id(value.as_ref())?);
                }
                BOOT_COURSE_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{BOOT_COURSE_ARG} requires a course id");
                    };
                    options.boot_course_id = Some(parse_boot_course_id(value.as_ref())?);
                }
                "--renderer" => {
                    let Some(value) = args.next() else {
                        bail!("--renderer requires a backend (vulkan, metal, dx12, gl, auto)");
                    };
                    options.renderer = Some(parse_renderer_backend(value.as_ref())?);
                }
                PRACTICE_SHORT_ARG | PRACTICE_ARG => options.boot_practice = true,
                PRACTICE_START_MS_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{PRACTICE_START_MS_ARG} requires milliseconds");
                    };
                    options.practice_start_ms =
                        Some(parse_practice_ms(value.as_ref(), PRACTICE_START_MS_ARG)?);
                }
                PRACTICE_END_MS_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{PRACTICE_END_MS_ARG} requires milliseconds");
                    };
                    options.practice_end_ms =
                        Some(parse_practice_ms(value.as_ref(), PRACTICE_END_MS_ARG)?);
                }
                LUA_SKIN_RUNTIME_ARG => {
                    let Some(value) = args.next() else {
                        bail!("{LUA_SKIN_RUNTIME_ARG} requires a mode (auto or compat)");
                    };
                    options.lua_skin_runtime_mode = parse_lua_skin_runtime_mode(value.as_ref())?;
                }
                _ if let Some(slot) = parse_beatoraja_replay_flag(arg) => {
                    options.boot_replay_slot = Some(slot);
                }
                _ if arg.starts_with('-') => bail!("unknown argument: {arg}"),
                _ => options.boot_play_path = Some(arg.to_string()),
            }
        }

        if options.viewer_stop
            && (options.viewer_play
                || options.boot_play_path.is_some()
                || options.start_measure.is_some())
        {
            bail!("{VIEWER_STOP_SHORT_ARG} cannot be combined with viewer play arguments");
        }
        if options.viewer_play && options.boot_play_path.is_none() {
            bail!("{VIEWER_PLAY_SHORT_ARG} requires a chart path");
        }

        Ok(options)
    }
}
use super::help::{
    parse_beatoraja_replay_flag, parse_boot_course_id, parse_boot_course_replay_id,
    parse_boot_replay_slot, parse_lua_skin_runtime_mode, parse_practice_ms, parse_renderer_backend,
    parse_smoke_exit_after_frames_value, parse_smoke_exit_after_play_frames_value,
    parse_smoke_exit_after_result_frames_value, parse_smoke_screenshot_path, parse_start_measure,
};
use super::*;
