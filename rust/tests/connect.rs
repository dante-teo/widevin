use flate2::{Compression, write::GzEncoder};
use futures_util::StreamExt;
use std::io::Write;
use widevin::{
    CONNECT_COMPRESSED_FLAG, CONNECT_END_STREAM_FLAG, DevinError, MAX_CONNECT_FRAME_PAYLOAD,
    decode_connect_frames, encode_connect_frame, read_connect_trailer_error,
};

#[tokio::test]
async fn compressed_frames_decode_across_partial_chunks() {
    let frame = encode_connect_frame(b"hello", true, false).expect("encode");
    assert_eq!(frame[0], CONNECT_COMPRESSED_FLAG);
    let chunks = futures_util::stream::iter(vec![Ok(frame[..3].to_vec()), Ok(frame[3..].to_vec())]);
    let decoded = decode_connect_frames(chunks)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("decode");
    assert_eq!(decoded[0].payload, b"hello");
}

#[tokio::test]
async fn trailers_gzip_and_malformed_streams_are_handled() {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(br#"{"error":{"code":"x","message":"bad"}}"#)
        .expect("gzip write");
    let payload = encoder.finish().expect("gzip finish");
    let trailer = encode_connect_frame(&payload, false, true).expect("frame");
    let trailer = [
        vec![CONNECT_COMPRESSED_FLAG | CONNECT_END_STREAM_FLAG],
        trailer[1..].to_vec(),
    ]
    .concat();
    let frames = decode_connect_frames(futures_util::stream::iter(vec![Ok(trailer)]))
        .collect::<Vec<_>>()
        .await;
    let frame = frames[0].as_ref().expect("decode trailer");
    assert!(frame.end_stream);
    assert_eq!(
        read_connect_trailer_error(&frame.payload).as_deref(),
        Some("Devin stream error x: bad")
    );

    let partial = decode_connect_frames(futures_util::stream::iter(vec![Ok(vec![0, 0])]))
        .collect::<Vec<_>>()
        .await;
    assert!(matches!(
        partial.last(),
        Some(Err(DevinError::Protocol { .. }))
    ));

    let overlarge = [
        vec![0],
        u32::try_from(MAX_CONNECT_FRAME_PAYLOAD + 1)
            .expect("frame cap fits u32")
            .to_be_bytes()
            .to_vec(),
    ]
    .concat();
    let frames = decode_connect_frames(futures_util::stream::iter(vec![Ok(overlarge)]))
        .collect::<Vec<_>>()
        .await;
    assert!(matches!(
        frames.last(),
        Some(Err(DevinError::Protocol { .. }))
    ));
}
