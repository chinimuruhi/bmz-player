use crate::skin_loader::*;

pub fn default_skin_root() -> PathBuf {
    resolve_app_paths()
        .map(|paths| paths.default_skin_root())
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/default"))
}

pub fn default_skin_root_from_paths(app_paths: &AppPaths) -> PathBuf {
    app_paths.default_skin_root()
}

pub fn apply_default_skin(renderer: &mut Renderer) -> Result<()> {
    let app_paths = resolve_app_paths()?;
    apply_default_skin_from_paths(renderer, &app_paths)
}

pub fn apply_default_skin_from_paths(renderer: &mut Renderer, app_paths: &AppPaths) -> Result<()> {
    let manifest = load_default_skin_into_renderer_from_paths(renderer, app_paths)?;
    let skin_path = default_play_skin_document_path_from_paths(app_paths, KeyMode::K7);
    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play)?;
    install_decoded_skin(renderer, decoded, manifest)
}

/// `profile.toml` の `[skin] play` 設定からスキンをロードする。
/// 空文字列 → デフォルト JSON スキン、`.json`/`.luaskin`/`.lua`/`.lr2skin`
/// 拡張子 → beatoraja スキンとして扱う。BMZ TOML skin directory は非対応。
pub fn apply_skin_from_config(
    renderer: &mut Renderer,
    app_paths: &AppPaths,
    play_skin_path: &str,
) -> Result<()> {
    if play_skin_path.is_empty() {
        return apply_default_skin_from_paths(renderer, app_paths);
    }
    let path = app_paths.resolve_path_ref(play_skin_path)?;
    if is_decodable_skin_path(&path) {
        apply_beatoraja_json_skin(renderer, &path)
    } else {
        anyhow::bail!(
            "unsupported skin path (BMZ TOML skin directories are no longer supported): {}",
            path.display()
        )
    }
}

pub fn apply_beatoraja_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Play)
}

pub fn apply_beatoraja_select_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Select)
}

pub fn apply_beatoraja_result_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Result)
}

pub fn apply_beatoraja_decide_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Decide)
}

pub(in crate::skin_loader) fn apply_beatoraja_json_skin_for_kind(
    renderer: &mut Renderer,
    skin_path: &Path,
    kind: SkinKind,
) -> Result<()> {
    let manifest = load_default_skin_into_renderer(renderer)?;
    let decoded = decode_beatoraja_skin(skin_path, kind)?;
    install_decoded_skin(renderer, decoded, manifest)
}

/// デフォルトスキンの manifest と PNG テクスチャを renderer に取り込む。
/// 起動時に 1 回だけ呼ばれることを想定 (同じテクスチャを複数回 upsert しても害は無いが無駄)。
pub fn load_default_skin_into_renderer(renderer: &mut Renderer) -> Result<SkinManifest> {
    let default_root = default_skin_root();
    load_default_skin_root_into_renderer(renderer, &default_root)
}

pub fn load_default_skin_into_renderer_from_paths(
    renderer: &mut Renderer,
    app_paths: &AppPaths,
) -> Result<SkinManifest> {
    let default_root = default_skin_root_from_paths(app_paths);
    load_default_skin_root_into_renderer(renderer, &default_root)
}

pub(in crate::skin_loader) fn load_default_skin_root_into_renderer(
    renderer: &mut Renderer,
    default_root: &Path,
) -> Result<SkinManifest> {
    let manifest = default_skin_manifest_for_root(default_root);

    for texture in manifest.resolve_textures(default_root) {
        renderer.load_png_texture(texture.id, &texture.path).with_context(|| {
            format!(
                "failed to load default skin texture {}: {}",
                texture.id.0,
                texture.path.display()
            )
        })?;
    }
    Ok(manifest)
}
