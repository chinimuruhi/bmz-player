use super::*;

pub(in crate::skin) fn select_runtime_document_sources(
    base_sources: &HashMap<String, SkinDocumentTexture>,
    snapshot: &SelectSnapshot,
) -> HashMap<String, SkinDocumentTexture> {
    let mut sources = base_sources.clone();
    if snapshot.stage_background
        && let Some(source_size) = snapshot.stage_image_size
    {
        insert_runtime_document_source(&mut sources, "100", SELECT_STAGE_TEXTURE, source_size);
    }
    if snapshot.backbmp_image
        && let Some(source_size) = snapshot.backbmp_image_size
    {
        insert_runtime_document_source(&mut sources, "101", PLAY_BACKBMP_TEXTURE, source_size);
    }
    if snapshot.banner_image
        && let Some(source_size) = snapshot.banner_image_size
    {
        insert_runtime_document_source(&mut sources, "102", SELECT_BANNER_TEXTURE, source_size);
    }
    sources
}

pub(in crate::skin) fn static_runtime_document_sources(
    base_sources: &HashMap<String, SkinDocumentTexture>,
    state: &SkinDrawState,
) -> HashMap<String, SkinDocumentTexture> {
    let mut sources = base_sources.clone();
    if state.has_stagefile
        && let Some(source_size) = state.stagefile_image_size
    {
        insert_runtime_document_source(&mut sources, "100", SELECT_STAGE_TEXTURE, source_size);
    }
    if state.has_backbmp {
        insert_runtime_document_source(
            &mut sources,
            "101",
            PLAY_BACKBMP_TEXTURE,
            SkinImageSize { width: 1.0, height: 1.0 },
        );
    }
    sources
}

pub(in crate::skin) fn insert_runtime_document_source(
    sources: &mut HashMap<String, SkinDocumentTexture>,
    source_id: &str,
    texture: TextureId,
    source_size: SkinImageSize,
) {
    sources.insert(
        source_id.to_string(),
        SkinDocumentTexture {
            source_id: source_id.to_string(),
            texture: SkinTextureId(texture.0),
            source_size,
        },
    );
}
