use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use liblzma::read;
use liblzma::stream;
use liblzma::write;

#[test]
fn standard_files() {
    for file in Path::new("liblzma-sys/xz/tests/files").read_dir().unwrap() {
        let file = file.unwrap();
        if file.path().extension().and_then(|s| s.to_str()) != Some("xz") {
            continue;
        }

        let filename = file.file_name().into_string().unwrap();

        // This appears to be implementation-defined how it's handled
        if filename.contains("unsupported-check") {
            continue;
        }

        println!("testing {:?}", file.path());
        let mut contents = Vec::new();
        File::open(&file.path())
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        if filename.starts_with("bad") || filename.starts_with("unsupported") {
            test_bad(&contents);
        } else {
            test_good(&contents);
        }
    }
}

fn test_good(data: &[u8]) {
    let mut ret = Vec::new();
    read::XzDecoder::new_multi_decoder(data)
        .read_to_end(&mut ret)
        .unwrap();
    let mut w = write::XzDecoder::new_multi_decoder(ret);
    w.write_all(data).unwrap();
    w.finish().unwrap();
}

fn test_bad(data: &[u8]) {
    let mut ret = Vec::new();
    let stream = stream::Stream::new_stream_decoder(u64::MAX, stream::CONCATENATED).unwrap();
    let result = read::XzDecoder::new_stream(data, stream).read_to_end(&mut ret);
    assert!(result.is_err(), "{result:?}");
    let mut w = write::XzDecoder::new(ret);
    assert!(w.write_all(data).is_err() || w.finish().is_err());
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn impls_send_and_sync() {
    assert_send_sync::<stream::Stream>();
    assert_send_sync::<read::XzDecoder<&[u8]>>();
    assert_send_sync::<read::XzEncoder<&[u8]>>();
    assert_send_sync::<write::XzEncoder<&mut [u8]>>();
    assert_send_sync::<write::XzDecoder<&mut [u8]>>();
}
