use super::*;

#[test]
fn app_options_parse_beatoraja_style_boot_path() {
    let options = AppOptions::parse_args(["/music/song.bms"]).unwrap();
    assert_eq!(options.boot_play_path.as_deref(), Some("/music/song.bms"));
    assert!(!options.autoplay_on_start);
    assert_eq!(options.boot_replay_slot, None);

    let options = AppOptions::parse_args(["-a", "/music/song.bms"]).unwrap();
    assert!(options.autoplay_on_start);
    assert_eq!(options.boot_play_path.as_deref(), Some("/music/song.bms"));

    let options = AppOptions::parse_args(["-r3", "/music/song.bms"]).unwrap();
    assert_eq!(options.boot_replay_slot, Some(2));
    assert_eq!(options.boot_play_path.as_deref(), Some("/music/song.bms"));

    let options =
        AppOptions::parse_args(["--renderer", "vulkan", "-a", "-r1", "/music/song.bms"]).unwrap();
    assert_eq!(options.renderer, Some(RendererBackend::Vulkan));
    assert!(options.autoplay_on_start);
    assert_eq!(options.boot_replay_slot, Some(0));
    assert_eq!(options.boot_play_path.as_deref(), Some("/music/song.bms"));
}

#[test]
fn app_options_parse_practice_flags() {
    let options =
        AppOptions::parse_args(["-p", "--practice-start-ms=5000", "/music/song.bms"]).unwrap();
    assert!(options.boot_practice);
    assert_eq!(options.practice_start_ms, Some(5000));
    assert_eq!(options.practice_end_ms, None);
    assert_eq!(options.boot_play_path.as_deref(), Some("/music/song.bms"));

    let options =
        AppOptions::parse_args(["--practice", "--practice-end-ms", "120000", "/music/song.bms"])
            .unwrap();
    assert!(options.boot_practice);
    assert_eq!(options.practice_end_ms, Some(120_000));
}

#[test]
fn app_options_parse_lua_skin_runtime_mode() {
    let separated = AppOptions::parse_args(["--lua-skin-runtime", "compat"]).unwrap();
    assert_eq!(separated.lua_skin_runtime_mode, bmz_skin::LuaSkinRuntimeMode::Compat);

    let equals = AppOptions::parse_args(["--lua-skin-runtime=auto"]).unwrap();
    assert_eq!(equals.lua_skin_runtime_mode, bmz_skin::LuaSkinRuntimeMode::Auto);

    assert!(AppOptions::parse_args(["--lua-skin-runtime=fast"]).is_err());
    assert!(AppOptions::parse_args(["--lua-skin-runtime"]).is_err());
}

#[test]
fn parse_beatoraja_replay_flag_maps_slots() {
    assert_eq!(parse_beatoraja_replay_flag("-r1"), Some(0));
    assert_eq!(parse_beatoraja_replay_flag("-r4"), Some(3));
    assert_eq!(parse_beatoraja_replay_flag("-a"), None);
    assert_eq!(parse_beatoraja_replay_flag("-r5"), None);
}

#[test]
fn app_options_parse_flags() {
    let options = AppOptions::parse_args([
        "--boot-play-sample",
        "--boot-result-sample",
        "--autoplay-on-start",
        "--smoke-exit-after-frames",
        "12",
        "--smoke-exit-after-play-frames",
        "24",
        "--smoke-exit-after-result-frames",
        "120",
        "--smoke-exit-on-result",
    ])
    .unwrap();

    assert!(options.boot_play_sample);
    assert!(options.boot_result_sample);
    assert!(options.autoplay_on_start);
    assert_eq!(options.smoke_exit_after_frames, Some(12));
    assert_eq!(options.smoke_exit_after_play_frames, Some(24));
    assert_eq!(options.smoke_exit_after_result_frames, Some(120));
    assert!(options.smoke_exit_on_result);
}

#[test]
fn app_options_parse_equals_form() {
    let options = AppOptions::parse_args([
        "--smoke-exit-after-frames=3",
        "--smoke-exit-after-play-frames=30",
        "--smoke-exit-after-result-frames=60",
    ])
    .unwrap();

    assert_eq!(options.smoke_exit_after_frames, Some(3));
    assert_eq!(options.smoke_exit_after_play_frames, Some(30));
    assert_eq!(options.smoke_exit_after_result_frames, Some(60));
}

#[test]
fn app_options_parse_smoke_screenshot_defaults_to_three_frames() {
    let options = AppOptions::parse_args(["--smoke-screenshot", "/tmp/bmz.png"]).unwrap();

    assert_eq!(options.smoke_screenshot_path.as_deref(), Some("/tmp/bmz.png"));
    assert_eq!(options.smoke_exit_after_frames, Some(3));
}

#[test]
fn app_options_parse_smoke_screenshot_keeps_explicit_frame_count() {
    let options =
        AppOptions::parse_args(["--smoke-exit-after-frames=8", "--smoke-screenshot=/tmp/bmz.png"])
            .unwrap();

    assert_eq!(options.smoke_screenshot_path.as_deref(), Some("/tmp/bmz.png"));
    assert_eq!(options.smoke_exit_after_frames, Some(8));
}

#[test]
fn app_options_clamps_zero_frame_count_to_one() {
    let options = AppOptions::parse_args([
        "--smoke-exit-after-frames",
        "0",
        "--smoke-exit-after-play-frames",
        "0",
        "--smoke-exit-after-result-frames",
        "0",
    ])
    .unwrap();

    assert_eq!(options.smoke_exit_after_frames, Some(1));
    assert_eq!(options.smoke_exit_after_play_frames, Some(1));
    assert_eq!(options.smoke_exit_after_result_frames, Some(1));
}

#[test]
fn app_options_reject_invalid_arguments() {
    assert!(AppOptions::parse_args(["--unknown"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-exit-after-frames"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-exit-after-frames", "abc"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-exit-after-play-frames"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-exit-after-play-frames", "abc"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-exit-after-result-frames"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-exit-after-result-frames", "abc"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-screenshot"]).is_err());
    assert!(AppOptions::parse_args(["--smoke-screenshot", ""]).is_err());
}

#[test]
fn help_args_are_detected() {
    assert!(args_request_help(["--help"]));
    assert!(args_request_help(["-h"]));
    assert!(args_request_help(["--boot-play-sample", "--help"]));
    assert!(!args_request_help(["--boot-play-sample"]));
}

#[test]
fn help_text_lists_supported_options() {
    let help = app_help_text();

    assert!(help.contains("--boot-play-sample"));
    assert!(help.contains("--boot-result-sample"));
    assert!(help.contains("--autoplay-on-start"));
    assert!(help.contains("--lua-skin-runtime <auto|compat>"));
    assert!(help.contains("--smoke-exit-after-frames"));
    assert!(help.contains("--smoke-exit-after-play-frames"));
    assert!(help.contains("--smoke-exit-after-result-frames"));
    assert!(help.contains("--smoke-exit-on-result"));
    assert!(help.contains("--smoke-screenshot"));
    assert!(help.contains("--renderer"));
    assert!(help.contains("table add"));
    assert!(help.contains("table list"));
    assert!(help.contains("table fetch"));
    assert!(help.contains("course import"));
    assert!(help.contains("course list"));
    assert!(help.contains("profile create"));
    assert!(help.contains("profile copy"));
    assert!(help.contains("ir upload-local"));
    assert!(help.contains("ir download-scores"));
    assert!(help.contains("upload-local [--dry-run] [--limit N] [--sync] [--all]"));
    assert!(help.contains("ir attest-submitted"));
    assert!(help.contains("ir cleanup-imported [--provider KEY] [--apply]"));
    assert!(help.contains("ir cleanup-duplicate <HISTORY_ID> [--provider KEY] --apply"));
}

#[test]
fn parse_command_routes_table_subcommands() {
    assert_eq!(
        parse_command(["table", "add", "https://example.com/"]).unwrap(),
        Command::Table(TableCommand::Add { url: "https://example.com/".to_string() })
    );
    assert_eq!(parse_command(["table", "list"]).unwrap(), Command::Table(TableCommand::List));
    assert_eq!(
        parse_command(["table", "fetch"]).unwrap(),
        Command::Table(TableCommand::Fetch { url: None })
    );
    assert_eq!(
        parse_command(["table", "fetch", "https://example.com/"]).unwrap(),
        Command::Table(TableCommand::Fetch { url: Some("https://example.com/".to_string()) })
    );
}

#[test]
fn parse_command_routes_app_flags() {
    assert!(matches!(
        parse_command(["--boot-play-sample"]).unwrap(),
        Command::Run(opts) if opts.boot_play_sample
    ));
    assert!(matches!(
        parse_command(["--boot-result-sample"]).unwrap(),
        Command::Run(opts) if opts.boot_result_sample
    ));
    assert!(matches!(parse_command([] as [&str; 0]).unwrap(), Command::Run(_)));
}

#[test]
fn parse_command_routes_ir_upload_local_flags() {
    assert_eq!(
        parse_command([
            "ir",
            "upload-local",
            "--limit=123",
            "--provider",
            "bmz-official",
            "--all",
            "--resend",
            "--include-course-stages",
            "--include-replay",
        ])
        .unwrap(),
        Command::Ir(IrCommand::UploadLocal {
            provider: Some("bmz-official".to_string()),
            limit: 123,
            dry_run: false,
            sync: true,
            all: true,
            resend: true,
            include_course_stages: true,
            include_replay: true,
        })
    );
}

#[test]
fn parse_command_accepts_rian_login_id() {
    assert_eq!(
        parse_command([
            "ir",
            "login",
            "--provider",
            "rian-ir",
            "--id",
            "player",
            "--password",
            "secret",
        ])
        .unwrap(),
        Command::Ir(IrCommand::Login {
            email: "player".to_string(),
            password: Some("secret".to_string()),
            base_url: None,
            provider: "rian-ir".to_string(),
        })
    );
}

#[test]
fn parse_command_rejects_invalid_ir_upload_local_flags() {
    assert!(parse_command(["ir", "upload-local", "--limit", "0"]).is_err());
    assert!(parse_command(["ir", "upload-local", "--limit", "abc"]).is_err());
    assert!(parse_command(["ir", "upload-local", "--provider"]).is_err());
    assert!(parse_command(["ir", "upload-local", "--all", "--dry-run"]).is_err());
    assert!(parse_command(["ir", "upload-local", "--all", "--no-sync"]).is_err());
    assert!(parse_command(["ir", "upload-local", "--unknown"]).is_err());
}

#[test]
fn parse_command_routes_ir_download_scores_flags() {
    assert_eq!(
        parse_command([
            "ir",
            "download-scores",
            "--dry-run",
            "--limit",
            "25",
            "--provider=bmz-official",
        ])
        .unwrap(),
        Command::Ir(IrCommand::DownloadScores {
            provider: Some("bmz-official".to_string()),
            limit: 25,
            dry_run: true,
        })
    );
}

#[test]
fn parse_command_rejects_invalid_ir_download_scores_flags() {
    assert!(parse_command(["ir", "download-scores", "--limit", "0"]).is_err());
    assert!(parse_command(["ir", "download-scores", "--provider"]).is_err());
    assert!(parse_command(["ir", "download-scores", "--unknown"]).is_err());
}

#[test]
fn parse_command_routes_ir_attest_submitted_flags() {
    assert_eq!(
        parse_command(["ir", "attest-submitted", "--provider=bmz", "--all"]).unwrap(),
        Command::Ir(IrCommand::AttestSubmitted {
            provider: Some("bmz".to_string()),
            sync: true,
            all: true,
        })
    );
    assert!(parse_command(["ir", "attest-submitted", "--all", "--no-sync"]).is_err());
}

#[test]
fn parse_command_routes_ir_cleanup_imported_flags() {
    assert_eq!(
        parse_command(["ir", "cleanup-imported", "--provider=bmz", "--apply"]).unwrap(),
        Command::Ir(IrCommand::CleanupImported { provider: Some("bmz".to_string()), apply: true })
    );
    assert!(parse_command(["ir", "cleanup-imported", "--provider"]).is_err());
    assert!(parse_command(["ir", "cleanup-imported", "--unknown"]).is_err());
}

#[test]
fn parse_command_routes_ir_cleanup_duplicate() {
    assert_eq!(
        parse_command(["ir", "cleanup-duplicate", "42", "--provider=bmz", "--apply"]).unwrap(),
        Command::Ir(IrCommand::CleanupDuplicate {
            history_id: 42,
            provider: Some("bmz".to_string()),
            apply: true,
        })
    );
    assert_eq!(
        parse_command(["ir", "cleanup-duplicate", "7", "--provider", "bmz", "--apply"]).unwrap(),
        Command::Ir(IrCommand::CleanupDuplicate {
            history_id: 7,
            provider: Some("bmz".to_string()),
            apply: true,
        })
    );
}

#[test]
fn parse_command_rejects_invalid_ir_cleanup_duplicate() {
    assert!(parse_command(["ir", "cleanup-duplicate"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "0", "--apply"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "-1", "--apply"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "abc", "--apply"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "1"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "1", "--provider"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "1", "--unknown", "--apply"]).is_err());
    assert!(parse_command(["ir", "cleanup-duplicate", "1", "extra", "--apply"]).is_err());
}

#[test]
fn parse_command_rejects_unknown_table_subcommand() {
    assert!(parse_command(["table", "remove"]).is_err());
    assert!(parse_command(["table"]).is_err());
    assert!(parse_command(["table", "add"]).is_err());
}

#[test]
fn parse_command_routes_songs_subcommands() {
    assert_eq!(
        parse_command(["songs", "add", "/bms"]).unwrap(),
        Command::Songs(SongsCommand::Add {
            path: "/bms".to_string(),
            recursive: true,
            enabled: true,
        })
    );
    assert_eq!(
        parse_command(["songs", "add", "/bms", "--no-recursive", "--disabled"]).unwrap(),
        Command::Songs(SongsCommand::Add {
            path: "/bms".to_string(),
            recursive: false,
            enabled: false,
        })
    );
    assert_eq!(parse_command(["songs", "list"]).unwrap(), Command::Songs(SongsCommand::List));
    assert_eq!(
        parse_command(["songs", "load"]).unwrap(),
        Command::Songs(SongsCommand::Load { target: None })
    );
    assert_eq!(
        parse_command(["songs", "load", "my-folder"]).unwrap(),
        Command::Songs(SongsCommand::Load { target: Some("my-folder".to_string()) })
    );
    assert_eq!(
        parse_command(["songs", "reload"]).unwrap(),
        Command::Songs(SongsCommand::Reload { target: None })
    );
    assert_eq!(
        parse_command(["songs", "reload", "/bms"]).unwrap(),
        Command::Songs(SongsCommand::Reload { target: Some("/bms".to_string()) })
    );
}

#[test]
fn parse_command_routes_course_subcommands() {
    assert_eq!(
        parse_command(["course", "import", "/course"]).unwrap(),
        Command::Course(CourseCommand::Import { path: "/course".to_string() })
    );
    assert_eq!(parse_command(["course", "list"]).unwrap(), Command::Course(CourseCommand::List));
}

#[test]
fn parse_command_routes_profile_subcommands() {
    assert_eq!(parse_command(["profile", "list"]).unwrap(), Command::Profile(ProfileCommand::List));
    assert_eq!(
        parse_command(["profile", "current"]).unwrap(),
        Command::Profile(ProfileCommand::Current)
    );
    assert_eq!(
        parse_command(["profile", "use", "alt"]).unwrap(),
        Command::Profile(ProfileCommand::Use { id: "alt".to_string() })
    );
    assert_eq!(
        parse_command(["profile", "create", "alt", "--name", "Alt", "--activate"]).unwrap(),
        Command::Profile(ProfileCommand::Create {
            id: "alt".to_string(),
            display_name: Some("Alt".to_string()),
            activate: true,
        })
    );
    assert_eq!(
        parse_command(["profile", "copy", "default", "alt", "--display-name", "Alt Copy",])
            .unwrap(),
        Command::Profile(ProfileCommand::Copy {
            source_id: "default".to_string(),
            target_id: "alt".to_string(),
            display_name: Some("Alt Copy".to_string()),
            activate: false,
        })
    );
}

#[test]
fn parse_command_rejects_invalid_profile_subcommands() {
    assert!(parse_command(["profile"]).is_err());
    assert!(parse_command(["profile", "create"]).is_err());
    assert!(parse_command(["profile", "copy", "default"]).is_err());
    assert!(parse_command(["profile", "use"]).is_err());
    assert!(parse_command(["profile", "delete", "default"]).is_err());
}

#[test]
fn parse_command_rejects_unknown_course_subcommand() {
    assert!(parse_command(["course", "remove"]).is_err());
    assert!(parse_command(["course"]).is_err());
    assert!(parse_command(["course", "import"]).is_err());
}

#[test]
fn parse_command_rejects_unknown_songs_subcommand() {
    assert!(parse_command(["songs", "remove"]).is_err());
    assert!(parse_command(["songs"]).is_err());
    assert!(parse_command(["songs", "add"]).is_err());
}

#[test]
fn help_text_lists_songs_subcommands() {
    let help = app_help_text();
    assert!(help.contains("songs add"));
    assert!(help.contains("songs list"));
    assert!(help.contains("songs load"));
    assert!(help.contains("songs reload"));
}

#[test]
fn app_options_parse_renderer_arg() {
    let options = AppOptions::parse_args(["--renderer", "vulkan"]).unwrap();
    assert_eq!(options.renderer, Some(RendererBackend::Vulkan));

    let options = AppOptions::parse_args(["--renderer=metal"]).unwrap();
    assert_eq!(options.renderer, Some(RendererBackend::Metal));

    assert!(AppOptions::parse_args(["--renderer", "invalid"]).is_err());
}

#[test]
fn app_options_parse_boot_replay_slot_arg() {
    let options = AppOptions::parse_args(["--boot-replay", "2"]).unwrap();
    assert_eq!(options.boot_replay_slot, Some(1));

    let options = AppOptions::parse_args(["--boot-replay", "4"]).unwrap();
    assert_eq!(options.boot_replay_slot, Some(3));
}

#[test]
fn app_options_parse_boot_replay_equals_form() {
    let options = AppOptions::parse_args(["--boot-replay=1"]).unwrap();
    assert_eq!(options.boot_replay_slot, Some(0));
}

#[test]
fn app_options_reject_boot_replay_out_of_range() {
    assert!(AppOptions::parse_args(["--boot-replay", "0"]).is_err());
    assert!(AppOptions::parse_args(["--boot-replay", "5"]).is_err());
    assert!(AppOptions::parse_args(["--boot-replay"]).is_err());
    assert!(AppOptions::parse_args(["--boot-replay", "abc"]).is_err());
}

#[test]
fn help_text_lists_boot_replay() {
    let help = app_help_text();
    assert!(help.contains("--boot-replay"));
}

#[test]
fn app_options_parse_boot_course_replay_id() {
    let options = AppOptions::parse_args(["--boot-course-replay", "42"]).unwrap();
    assert_eq!(options.boot_course_replay_id, Some(42));

    let options = AppOptions::parse_args(["--boot-course-replay=7"]).unwrap();
    assert_eq!(options.boot_course_replay_id, Some(7));
}

#[test]
fn app_options_reject_invalid_boot_course_replay_id() {
    assert!(AppOptions::parse_args(["--boot-course-replay"]).is_err());
    assert!(AppOptions::parse_args(["--boot-course-replay", "0"]).is_err());
    assert!(AppOptions::parse_args(["--boot-course-replay", "-1"]).is_err());
    assert!(AppOptions::parse_args(["--boot-course-replay", "abc"]).is_err());
}

#[test]
fn help_text_lists_boot_course_replay() {
    let help = app_help_text();
    assert!(help.contains("--boot-course-replay"));
}

#[test]
fn app_options_parse_boot_course_id() {
    let options = AppOptions::parse_args(["--boot-course", "42"]).unwrap();
    assert_eq!(options.boot_course_id, Some(42));

    let options = AppOptions::parse_args(["--boot-course=7"]).unwrap();
    assert_eq!(options.boot_course_id, Some(7));
}

#[test]
fn app_options_reject_invalid_boot_course_id() {
    assert!(AppOptions::parse_args(["--boot-course"]).is_err());
    assert!(AppOptions::parse_args(["--boot-course", "0"]).is_err());
    assert!(AppOptions::parse_args(["--boot-course", "-1"]).is_err());
    assert!(AppOptions::parse_args(["--boot-course", "abc"]).is_err());
}

#[test]
fn help_text_lists_boot_course() {
    let help = app_help_text();
    assert!(help.contains("--boot-course "));
}

#[test]
fn parse_command_routes_course_history() {
    assert_eq!(
        parse_command(["course", "history", "42"]).unwrap(),
        Command::Course(CourseCommand::History { course_id: 42, limit: 10 }),
    );
    assert_eq!(
        parse_command(["course", "history", "42", "--limit", "5"]).unwrap(),
        Command::Course(CourseCommand::History { course_id: 42, limit: 5 }),
    );
    assert_eq!(
        parse_command(["course", "history", "42", "--limit=20"]).unwrap(),
        Command::Course(CourseCommand::History { course_id: 42, limit: 20 }),
    );
}

#[test]
fn parse_command_rejects_invalid_course_history() {
    assert!(parse_command(["course", "history"]).is_err());
    assert!(parse_command(["course", "history", "0"]).is_err());
    assert!(parse_command(["course", "history", "-1"]).is_err());
    assert!(parse_command(["course", "history", "abc"]).is_err());
    assert!(parse_command(["course", "history", "1", "--limit"]).is_err());
    assert!(parse_command(["course", "history", "1", "--limit=0"]).is_err());
    assert!(parse_command(["course", "history", "1", "--unknown"]).is_err());
}

#[test]
fn help_text_lists_course_history() {
    let help = app_help_text();
    assert!(help.contains("course history"));
}

#[test]
fn parse_command_routes_course_attempt() {
    assert_eq!(
        parse_command(["course", "attempt", "7"]).unwrap(),
        Command::Course(CourseCommand::Attempt { score_id: 7 }),
    );
}

#[test]
fn parse_command_rejects_invalid_course_attempt() {
    assert!(parse_command(["course", "attempt"]).is_err());
    assert!(parse_command(["course", "attempt", "0"]).is_err());
    assert!(parse_command(["course", "attempt", "-1"]).is_err());
    assert!(parse_command(["course", "attempt", "abc"]).is_err());
    assert!(parse_command(["course", "attempt", "1", "--unknown"]).is_err());
}

#[test]
fn help_text_lists_course_attempt() {
    let help = app_help_text();
    assert!(help.contains("course attempt"));
}
