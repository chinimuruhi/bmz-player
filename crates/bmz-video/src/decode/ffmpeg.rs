use super::*;

pub(crate) fn select_video_stream(
    ictx: &ffmpeg_next::format::context::Input,
) -> Result<SelectedVideoStream> {
    let best = ictx
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .ok_or_else(|| anyhow::anyhow!("no video stream found"))?;
    let best_index = best.index();
    let mut candidates = Vec::new();
    for stream in ictx.streams() {
        let params = stream.parameters();
        if params.medium() != ffmpeg_next::media::Type::Video {
            continue;
        }
        candidates.push((stream.index(), video_stream_bit_rate(&params), params));
    }
    let selected_index = choose_beatoraja_video_stream(
        best_index,
        candidates.iter().map(|(index, bitrate, _)| (*index, *bitrate)),
    );
    let (stream_index, codec_params) = candidates
        .into_iter()
        .find_map(|(index, _, params)| (index == selected_index).then_some((index, params)))
        .ok_or_else(|| anyhow::anyhow!("selected video stream not found"))?;
    let stream = ictx
        .stream(stream_index)
        .ok_or_else(|| anyhow::anyhow!("selected video stream not available"))?;
    let tb = stream.time_base();
    tracing::debug!(stream_index, best_index, "selected video stream for BGA decode");
    Ok(SelectedVideoStream {
        index: stream_index,
        time_base_num: tb.numerator() as i64,
        time_base_den: tb.denominator() as i64,
        codec_params,
    })
}

pub(crate) fn open_video_decoder(
    selected: &SelectedVideoStream,
) -> Result<ffmpeg_next::decoder::Video> {
    let mut context =
        ffmpeg_next::codec::context::Context::from_parameters(selected.codec_params.clone())?;
    context.set_threading(ffmpeg_next::codec::threading::Config::kind(
        ffmpeg_next::codec::threading::Type::Frame,
    ));
    Ok(context.decoder().video()?)
}

pub(crate) fn rewind_video_decoder(
    ictx: &mut ffmpeg_next::format::context::Input,
    decoder: &mut ffmpeg_next::decoder::Video,
) -> Result<()> {
    ictx.seek(0, ..)?;
    decoder.flush();
    Ok(())
}

pub(crate) fn video_stream_bit_rate(params: &ffmpeg_next::codec::Parameters) -> usize {
    ffmpeg_next::codec::context::Context::from_parameters(params.clone())
        .and_then(|context| context.decoder().video())
        .map(|decoder| decoder.bit_rate())
        .unwrap_or(0)
}

pub(crate) fn choose_beatoraja_video_stream(
    best_index: usize,
    candidates: impl IntoIterator<Item = (usize, usize)>,
) -> usize {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by_key(|(index, _)| *index);
    let best_bitrate = candidates
        .iter()
        .find_map(|(index, bitrate)| (*index == best_index).then_some(*bitrate))
        .unwrap_or(0);
    if best_bitrate >= 10 {
        return best_index;
    }
    candidates
        .iter()
        .find_map(|(index, bitrate)| {
            (*index > best_index && *index <= 5 && *bitrate >= 10).then_some(*index)
        })
        .or_else(|| {
            candidates
                .iter()
                .find_map(|(index, bitrate)| (*index <= 5 && *bitrate >= 10).then_some(*index))
        })
        .unwrap_or(best_index)
}

pub(crate) fn rgba_frame_from_video(
    decoded: &ffmpeg_next::frame::Video,
    pts_us: i64,
) -> Result<DecodedFrame> {
    let mut scaler = None;
    rgba_frame_from_video_with_scaler(decoded, pts_us, &mut scaler, None)
}

pub(crate) fn rgba_frame_from_video_with_scaler(
    decoded: &ffmpeg_next::frame::Video,
    pts_us: i64,
    scaler: &mut Option<ffmpeg_next::software::scaling::context::Context>,
    rgba_buffer: Option<Vec<u8>>,
) -> Result<DecodedFrame> {
    let w = decoded.width();
    let h = decoded.height();

    if scaler.is_none() {
        *scaler = Some(ffmpeg_next::software::scaling::context::Context::get(
            decoded.format(),
            w,
            h,
            ffmpeg_next::format::Pixel::RGBA,
            w,
            h,
            ffmpeg_next::software::scaling::flag::Flags::FAST_BILINEAR,
        )?);
    }

    let mut rgba_frame = ffmpeg_next::frame::Video::empty();
    scaler.as_mut().unwrap().run(decoded, &mut rgba_frame)?;

    let data = rgba_frame.data(0);
    let stride = rgba_frame.stride(0);
    let row_bytes = (w as usize) * 4;
    let rgba = copy_rgba_frame_data_with_buffer(data, stride, row_bytes, h as usize, rgba_buffer);

    Ok(DecodedFrame { pts_us, rgba, width: w, height: h })
}

#[cfg(test)]
pub(crate) fn copy_rgba_frame_data(
    data: &[u8],
    stride: usize,
    row_bytes: usize,
    rows: usize,
) -> Vec<u8> {
    copy_rgba_frame_data_with_buffer(data, stride, row_bytes, rows, None)
}

pub(crate) fn copy_rgba_frame_data_with_buffer(
    data: &[u8],
    stride: usize,
    row_bytes: usize,
    rows: usize,
    rgba_buffer: Option<Vec<u8>>,
) -> Vec<u8> {
    let total_bytes = row_bytes.saturating_mul(rows);
    let mut rgba = rgba_buffer.unwrap_or_default();
    rgba.clear();
    if stride == row_bytes
        && let Some(contiguous) = data.get(..total_bytes)
    {
        rgba.extend_from_slice(contiguous);
        return rgba;
    }

    rgba.resize(total_bytes, 0);
    for row in 0..rows {
        let src_start = row.saturating_mul(stride);
        let dst_start = row * row_bytes;
        let Some(src) = data.get(src_start..src_start + row_bytes) else {
            break;
        };
        let dst = &mut rgba[dst_start..dst_start + row_bytes];
        dst.copy_from_slice(src);
    }
    rgba
}
